OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_supervised_renderer_generation_1_entry)

PHDRS
{
    image PT_LOAD FLAGS(5);
}

SECTIONS
{
    renderer_page_size = 0x1000;
    renderer_image_base = 0x0000600000001000;
    renderer_generation_2_entry = renderer_image_base + 0x200;
    renderer_image_limit = renderer_image_base + 0x40000;
    renderer_mailbox = 0x0000600000080000;
    renderer_stack_base = 0x0000600000090000;
    renderer_stack_top = renderer_stack_base + 0x4000;

    . = renderer_image_base;
    anomalous_supervised_renderer_image_start = .;

    .renderer_generation_1 : ALIGN(16)
    {
        KEEP(*(.text.renderer_generation_1_entry))
    } :image

    . = renderer_generation_2_entry;
    .renderer_generation_2 : ALIGN(16)
    {
        KEEP(*(.text.renderer_generation_2_entry))
    } :image

    .text : ALIGN(16)
    {
        KEEP(*(.text.renderer_rite))
        *(.text .text.*)
        *(.ltext .ltext.*)
    } :image

    .renderer_capsule : ALIGN(16)
    {
        anomalous_supervised_renderer_capsule_start = .;
        KEEP(*(.rodata.renderer_capsule))
        anomalous_supervised_renderer_capsule_end = .;
    } :image

    .rodata : ALIGN(16)
    {
        *(.rodata .rodata.*)
        *(.lrodata .lrodata.*)
        *(.data.rel.ro .data.rel.ro.*)
    } :image

    anomalous_supervised_renderer_image_payload_end = .;
    . = ALIGN(renderer_page_size);
    anomalous_supervised_renderer_image_end = .;

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

ASSERT(anomalous_supervised_renderer_image_end - anomalous_supervised_renderer_image_start <= 0x40000,
       "supervised renderer image exceeds 64-page cap")
ASSERT(anomalous_supervised_renderer_generation_1_entry == renderer_image_base,
       "generation-1 renderer entry moved")
ASSERT(anomalous_supervised_renderer_generation_2_entry == renderer_generation_2_entry,
       "generation-2 renderer entry moved")
ASSERT(anomalous_supervised_renderer_capsule_end - anomalous_supervised_renderer_capsule_start == 416,
       "supervised renderer capsule has the wrong size")
ASSERT((anomalous_supervised_renderer_image_start % renderer_page_size) == 0,
       "supervised renderer image start is not page-aligned")
ASSERT((anomalous_supervised_renderer_image_end % renderer_page_size) == 0,
       "supervised renderer image end is not page-aligned")
ASSERT(anomalous_supervised_renderer_image_end <= renderer_image_limit,
       "supervised renderer image crosses its 64-page extent")
ASSERT(renderer_image_limit < renderer_mailbox,
       "supervised renderer image cap overlaps its mailbox guard")
ASSERT((renderer_mailbox % renderer_page_size) == 0,
       "supervised renderer mailbox is not page-aligned")
ASSERT(renderer_mailbox + renderer_page_size < renderer_stack_base,
       "supervised renderer mailbox overlaps its stack guard")
ASSERT((renderer_stack_base % renderer_page_size) == 0,
       "supervised renderer stack is not page-aligned")
ASSERT((renderer_stack_top % renderer_page_size) == 0,
       "supervised renderer stack top is not page-aligned")
ASSERT(SIZEOF(.forbidden_writable) == 0,
       "supervised renderer unexpectedly requires writable image storage")
