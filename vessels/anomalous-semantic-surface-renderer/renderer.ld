OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_semantic_surface_renderer_entry)

PHDRS
{
    image PT_LOAD FLAGS(5);
}

SECTIONS
{
    renderer_page_size = 0x1000;
    renderer_image_base = 0x0000600000001000;
    renderer_image_limit = renderer_image_base + 0x40000;
    renderer_mailbox = 0x0000600000080000;
    renderer_stack_base = 0x0000600000090000;
    renderer_stack_top = renderer_stack_base + 0x4000;

    . = renderer_image_base;
    anomalous_semantic_surface_renderer_image_start = .;

    .renderer_entry : ALIGN(16)
    {
        KEEP(*(.text.semantic_surface_renderer_entry))
    } :image

    .text : ALIGN(16)
    {
        KEEP(*(.text.semantic_surface_renderer_rite))
        *(.text .text.*)
        *(.ltext .ltext.*)
    } :image

    .rodata : ALIGN(16)
    {
        *(.rodata .rodata.*)
        *(.lrodata .lrodata.*)
        *(.data.rel.ro .data.rel.ro.*)
    } :image

    anomalous_semantic_surface_renderer_image_payload_end = .;
    . = ALIGN(renderer_page_size);
    anomalous_semantic_surface_renderer_image_end = .;

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

ASSERT(anomalous_semantic_surface_renderer_image_end - anomalous_semantic_surface_renderer_image_start <= 0x40000,
       "semantic-surface renderer image exceeds 64-page cap")
ASSERT(anomalous_semantic_surface_renderer_entry == renderer_image_base,
       "semantic-surface renderer entry moved")
ASSERT((anomalous_semantic_surface_renderer_image_start % renderer_page_size) == 0,
       "semantic-surface renderer image start is not page-aligned")
ASSERT((anomalous_semantic_surface_renderer_image_end % renderer_page_size) == 0,
       "semantic-surface renderer image end is not page-aligned")
ASSERT(anomalous_semantic_surface_renderer_image_end <= renderer_image_limit,
       "semantic-surface renderer image crosses its 64-page extent")
ASSERT(renderer_image_limit < renderer_mailbox,
       "semantic-surface renderer image cap overlaps its mailbox guard")
ASSERT(renderer_mailbox + renderer_page_size < renderer_stack_base,
       "semantic-surface renderer mailbox overlaps its stack guard")
ASSERT((renderer_stack_top % renderer_page_size) == 0,
       "semantic-surface renderer stack top is not page-aligned")
ASSERT(SIZEOF(.forbidden_writable) == 0,
       "semantic-surface renderer unexpectedly requires writable image storage")
