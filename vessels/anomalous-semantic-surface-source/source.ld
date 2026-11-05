OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_semantic_surface_source_entry)

PHDRS
{
    image PT_LOAD FLAGS(5);
}

SECTIONS
{
    source_page_size = 0x1000;
    source_image_base = 0x0000600000001000;
    source_image_limit = source_image_base + 0x40000;
    source_mailbox = 0x0000600000080000;
    source_stack_base = 0x0000600000090000;
    source_stack_top = source_stack_base + 0x4000;

    . = source_image_base;
    anomalous_semantic_surface_source_image_start = .;

    .source_entry : ALIGN(16)
    {
        KEEP(*(.text.semantic_surface_source_entry))
    } :image

    .text : ALIGN(16)
    {
        KEEP(*(.text.semantic_surface_source_rite))
        *(.text .text.*)
        *(.ltext .ltext.*)
    } :image

    .semantic_surface_snapshot : ALIGN(16)
    {
        anomalous_semantic_surface_source_snapshot_start = .;
        KEEP(*(.rodata.semantic_surface_snapshot))
        anomalous_semantic_surface_source_snapshot_end = .;
    } :image

    .rodata : ALIGN(16)
    {
        *(.rodata .rodata.*)
        *(.lrodata .lrodata.*)
        *(.data.rel.ro .data.rel.ro.*)
    } :image

    anomalous_semantic_surface_source_image_payload_end = .;
    . = ALIGN(source_page_size);
    anomalous_semantic_surface_source_image_end = .;

    /DISCARD/ :
    {
        *(.comment)
        *(.eh_frame .eh_frame.*)
        *(.eh_frame_hdr)
        *(.got .got.*)
        *(.got.plt)
        *(.interp)
        *(.note .note.*)
        *(.rela .rela.*)
        *(.rel .rel.*)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu.hash)
        *(.hash)
        *(.plt .plt.*)
    }

    .forbidden_writable (NOLOAD) :
    {
        *(.data .data.*)
        *(.bss .bss.* COMMON)
        *(.tdata .tdata.*)
        *(.tbss .tbss.*)
    }
}

ASSERT(anomalous_semantic_surface_source_image_end - anomalous_semantic_surface_source_image_start <= 0x40000,
       "semantic-surface source image exceeds 64-page cap")
ASSERT(anomalous_semantic_surface_source_entry == source_image_base,
       "semantic-surface source entry moved")
ASSERT(anomalous_semantic_surface_source_snapshot_end - anomalous_semantic_surface_source_snapshot_start == 528,
       "semantic-surface source snapshot has the wrong size")
ASSERT((anomalous_semantic_surface_source_image_start % source_page_size) == 0,
       "semantic-surface source image start is not page-aligned")
ASSERT((anomalous_semantic_surface_source_image_end % source_page_size) == 0,
       "semantic-surface source image end is not page-aligned")
ASSERT(anomalous_semantic_surface_source_image_end <= source_image_limit,
       "semantic-surface source image crosses its 64-page extent")
ASSERT(source_image_limit < source_mailbox,
       "semantic-surface source image cap overlaps its mailbox guard")
ASSERT(source_mailbox + source_page_size < source_stack_base,
       "semantic-surface source mailbox overlaps its stack guard")
ASSERT((source_stack_top % source_page_size) == 0,
       "semantic-surface source stack top is not page-aligned")
ASSERT(SIZEOF(.forbidden_writable) == 0,
       "semantic-surface source unexpectedly requires writable image storage")
