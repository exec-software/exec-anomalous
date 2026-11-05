OUTPUT_FORMAT(elf64-x86-64)
ENTRY(kernel_main)

PHDRS {
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}

SECTIONS {
    . = 0x100000;
    kernel_image_start = .;

    .text : ALIGN(4K) {
        kernel_text_start = .;
        *(.text .text.*)
        . = ALIGN(4K);
        kernel_text_end = .;
    } :text

    .rodata : ALIGN(4K) {
        kernel_rodata_start = .;
        KEEP(*(.rodata.circle_test.payload))
        KEEP(*(.rodata.supervised_renderer.payload))
        *(.rodata .rodata.*)
        . = ALIGN(4K);
        kernel_rodata_end = .;
    } :text

    .data : ALIGN(4K) {
        kernel_data_start = .;
        *(.data .data.*)
        *(.data.rel.ro .data.rel.ro.*)
        *(.got .got.*)
        . = ALIGN(4K);
        kernel_data_end = .;
    } :data
    kernel_file_end = .;

    .relocation_guard : {
        *(.rel .rel.*)
        *(.rela .rela.*)
        *(.iplt .plt .plt.*)
        *(.igot .igot.*)
    } :data

    .bss (NOLOAD) : ALIGN(4K) {
        kernel_bss_start = .;
        *(COMMON)
        *(.bss .bss.*)
        . = ALIGN(4K);
        kernel_bss_end = .;
    } :data
    kernel_memory_end = .;

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

ASSERT(kernel_image_start == 0x100000, "Anomalous kernel must load at 1 MiB")
ASSERT(kernel_text_start % 4K == 0, "kernel text must be page aligned")
ASSERT(kernel_text_end % 4K == 0, "kernel text end must be page aligned")
ASSERT(kernel_rodata_start % 4K == 0, "kernel rodata must be page aligned")
ASSERT(kernel_rodata_end % 4K == 0, "kernel rodata end must be page aligned")
ASSERT(kernel_data_start % 4K == 0, "kernel data must be page aligned")
ASSERT(kernel_data_end % 4K == 0, "kernel data end must be page aligned")
ASSERT(kernel_bss_start % 4K == 0, "kernel bss must be page aligned")
ASSERT(kernel_bss_end % 4K == 0, "kernel bss end must be page aligned")
ASSERT(kernel_file_end > kernel_image_start, "Anomalous kernel must not be empty")
ASSERT(SIZEOF(.relocation_guard) == 0, "Anomalous kernel must be fully static")
ASSERT(
    kernel_file_end - kernel_image_start <= 384K,
    "Anomalous kernel exceeds the milestone-0 staging window"
)
ASSERT(
    kernel_memory_end - kernel_image_start <= 64M,
    "Anomalous kernel exceeds the bootstrap memory window"
)
ASSERT(
    kernel_main >= kernel_image_start && kernel_main < kernel_file_end,
    "kernel_main must be in the file-backed kernel image"
)
