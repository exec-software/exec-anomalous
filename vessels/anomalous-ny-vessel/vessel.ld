OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_ny_vessel_entry)
PHDRS {
    image PT_LOAD FLAGS(5);
    state PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0x400000;
    anomalous_ny_vessel_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.ny_vessel_entry))
        . = ALIGN(16);
        anomalous_ny_vessel_compiledcode_start = .;
        *(.text .text.*)
        anomalous_ny_vessel_compiledcode_end = .;
    } :image
    .rodata : ALIGN(16) { *(.rodata .rodata.*) } :image
    anomalous_ny_vessel_image_end = .;

    . = 0x800000;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_ny_vessel_state_start = .;
        KEEP(*(.bss.ny_vessel_state))
        *(.bss .bss.* COMMON)
        anomalous_ny_vessel_state_end = .;
    } :state

    /* The region the host installs and the vessel allocates from. Declared
       here so the extent is the linker's statement rather than a number
       repeated in the binding, and kept on its own page so vessel state
       cannot be reached by an allocation that overruns. */
    . = 0x801000;
    .arena (NOLOAD) : {
        anomalous_ny_vessel_arena_start = .;
        . += 0x1000;
        anomalous_ny_vessel_arena_end = .;
    } :state

    .forbidden (NOLOAD) : {
        *(.data .data.* .data.rel.ro .data.rel.ro.*)
        *(.tdata .tdata.* .tbss .tbss.*)
        *(.got .got.* .plt .plt.* .iplt .igot.*)
        *(.init .fini .init_array .init_array.* .fini_array .fini_array.*)
        *(.preinit_array .preinit_array.* .ctors .dtors)
        *(.interp .dynamic .dynsym .dynstr .hash .gnu.hash)
        *(.rela .rela.* .rel .rel.*)
    }
    /DISCARD/ : { *(.comment .eh_frame .eh_frame.* .eh_frame_hdr .note.GNU-stack .note.gnu.property) }
}
ASSERT(anomalous_ny_vessel_entry == 0x400000, "Ny vessel entry moved")
ASSERT(anomalous_ny_vessel_image_end - anomalous_ny_vessel_image_start <= 0x1000,
       "Ny vessel RX exceeds one page")
ASSERT(anomalous_ny_vessel_compiledcode_end > anomalous_ny_vessel_compiledcode_start,
       "Ny vessel compiled code is absent")
ASSERT(anomalous_ny_vessel_state_start == 0x800000, "Ny vessel state moved")
ASSERT(anomalous_ny_vessel_state_end - anomalous_ny_vessel_state_start <= 0x1000,
       "Ny vessel state exceeds one page")
ASSERT(anomalous_ny_vessel_arena_start == 0x801000, "Ny vessel arena moved")
ASSERT(anomalous_ny_vessel_arena_end - anomalous_ny_vessel_arena_start == 0x1000,
       "Ny vessel arena must be exactly one page")
ASSERT(SIZEOF(.forbidden) == 0, "Ny vessel has unsupported sections")
