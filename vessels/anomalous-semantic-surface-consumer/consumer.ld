OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_semantic_surface_consumer_entry)

PHDRS
{
    image PT_LOAD FLAGS(5);
}

SECTIONS
{
    consumer_page_size = 0x1000;
    consumer_image_base = 0x0000600000001000;
    consumer_image_limit = consumer_image_base + 0x40000;
    consumer_mailbox = 0x0000600000080000;
    consumer_stack_base = 0x0000600000090000;
    consumer_stack_top = consumer_stack_base + 0x4000;

    . = consumer_image_base;
    anomalous_semantic_surface_consumer_image_start = .;

    .consumer_entry : ALIGN(16)
    {
        KEEP(*(.text.semantic_surface_consumer_entry))
    } :image

    .text : ALIGN(16)
    {
        KEEP(*(.text.semantic_surface_consumer_rite))
        *(.text .text.*)
        *(.ltext .ltext.*)
    } :image

    .rodata : ALIGN(16)
    {
        *(.rodata .rodata.*)
        *(.lrodata .lrodata.*)
        *(.data.rel.ro .data.rel.ro.*)
    } :image

    anomalous_semantic_surface_consumer_image_payload_end = .;
    . = ALIGN(consumer_page_size);
    anomalous_semantic_surface_consumer_image_end = .;

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

ASSERT(anomalous_semantic_surface_consumer_image_end - anomalous_semantic_surface_consumer_image_start <= 0x40000,
       "semantic-surface consumer image exceeds 64-page cap")
ASSERT(anomalous_semantic_surface_consumer_entry == consumer_image_base,
       "semantic-surface consumer entry moved")
ASSERT((anomalous_semantic_surface_consumer_image_start % consumer_page_size) == 0,
       "semantic-surface consumer image start is not page-aligned")
ASSERT((anomalous_semantic_surface_consumer_image_end % consumer_page_size) == 0,
       "semantic-surface consumer image end is not page-aligned")
ASSERT(anomalous_semantic_surface_consumer_image_end <= consumer_image_limit,
       "semantic-surface consumer image crosses its 64-page extent")
ASSERT(consumer_image_limit < consumer_mailbox,
       "semantic-surface consumer image cap overlaps its mailbox guard")
ASSERT(consumer_mailbox + consumer_page_size < consumer_stack_base,
       "semantic-surface consumer mailbox overlaps its stack guard")
ASSERT((consumer_stack_top % consumer_page_size) == 0,
       "semantic-surface consumer stack top is not page-aligned")
ASSERT(SIZEOF(.forbidden_writable) == 0,
       "semantic-surface consumer unexpectedly requires writable image storage")
