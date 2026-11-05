OUTPUT_FORMAT(elf32-i386)
ENTRY(stage_1_start)

SECTIONS {
    . = 0x7c00;
    .boot : {
        KEEP(*(.boot))
    }

    .section_guard : {
        *(.text .text.*)
        *(.data .data.*)
        *(.bss .bss.*)
        *(.rel .rel.*)
        *(.rela .rela.*)
        *(.got .got.*)
        *(.iplt .plt .plt.*)
        *(.igot .igot.*)
    }

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

ASSERT(SIZEOF(.boot) == 512, "Threshold stage 1 must be exactly 512 bytes")
ASSERT(SIZEOF(.section_guard) == 0, "Threshold stage 1 contains an unexpected section")
