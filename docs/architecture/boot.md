# Threshold boot architecture

Threshold is the Anomalous-owned boot loader. It exists as a narrow set of
firmware frontends around a versioned handoff contract, not as a general-purpose
boot menu or foreign operating-system loader.

## Milestone 0 BIOS path

The implemented x86-64 path is:

1. Firmware loads stage_1 at physical address 0x7c00.
2. The 512-byte stage validates BIOS LBA extensions, reads the fixed manifest
   from LBA 1, validates its version and integrity hash, then loads and validates
   only stage_2 at physical address 0x10000.
3. Stage_2 validates the manifest independently, verifies the required x86-64
   CPU features, and reads the separately linked kernel into low-memory staging
   in chunks of at most 32 KiB. It verifies the kernel hash before continuing.
4. Stage_2 gathers the E820 memory map to its defined `EBX=0` end. Firmware
   carry, bad signature, a result shorter than 20 bytes, or a nonzero
   continuation at the 64-record capacity is a human-visible pre-kernel fatal
   error. It then enables and verifies A20, queries VBE controller information,
   requires VBE 2.0 or newer, and requests mode 0x118 with a linear framebuffer.
   VBE 2.x uses the legacy pitch and RGB fields; VBE 3.x or newer uses the
   linear-framebuffer fields. Threshold requires the supported, graphics, and
   linear-framebuffer mode attributes, then validates bit depth, pitch,
   geometry, byte extent, channel bounds and pairwise separation, four-GiB
   bounds, and separation from low and linked kernel memory before activating
   graphics or publishing the framebuffer flag. VGA text remains the fallback.
5. It masks NMI while no transition IDT exists, installs the GDT, enters 32-bit
   protected mode, copies the kernel to its linked address at 1 MiB, and
   establishes both zeroed-memory contracts.
6. It builds 2 MiB page mappings for the first four GiB, enables PAE and long
   mode, and calls the entry address declared by the manifest.
7. It passes a pointer to boot_omen in the first System V argument register. The
   Rust kernel validates the version-2 completeness attestation, copies the
   bounded descriptor table, and rejects malformed ranges before treating the
   map as allocation authority.
8. The kernel emits `boot.ready`; normal images then enter the bounded bootstrap
   command environment, while test-only smoke images report success through the
   QEMU debug-exit device before any interactive loop.

Before validating addresses referenced by the omen, the Rust kernel replaces
the loader descriptor state with its own GDT and TSS and installs a 32-vector
diagnostic exception IDT. Double fault and NMI have dedicated interrupt stacks.
NMI remains masked. Ordinary interrupts remain disabled until the accepted
`legacy_pc` backend has installed its returning gates, initialized source state,
masked and configured its controllers, and established acknowledgement paths;
only then does the normal kernel arm PIT and, when healthy, keyboard IRQs.

The build fails if stage_1 is not exactly 512 bytes, lacks the 0x55aa signature,
if an unexpected linker section appears, if any linked and flattened extent
disagrees, or if an address, size, entry point, or staging bound violates the
manifest contract. Stage_2 retains a 127-sector ceiling, but the kernel is no
longer coupled to it: the seed kernel may occupy up to the explicitly reserved
384 KiB staging window. The resulting disk is padded to 16 MiB for stable VM
geometry.

## Threshold manifest version 1

The 96-byte manifest occupies the beginning of LBA 1; the remainder of its
sector is zero. It records:

- magic, schema version, byte size, flags, and a header integrity hash;
- each image's LBA, exact file size, rounded sector count, and FNV-1a hash;
- the stage_2 load and entry addresses; and
- the kernel staging, linked load, memory extent, and entry addresses.

FNV-1a detects accidental corruption and inconsistent image assembly. It is not
an authenticity mechanism. A future verified-boot module must bind this
manifest and its payloads to cryptographic signatures and measured-boot policy.

The disk layout is one boot sector, one manifest sector, rounded stage_2
sectors, then independently rounded kernel sectors. Padding is never included
in an image hash.

## Diagnostic artifacts

The build preserves source-level DWARF and symbols without placing them in the
flat boot payload. `build/kernel.elf` is the linked kernel,
`build/kernel.debug` is its debugger companion, and
`build/threshold_stage_1.elf` plus `build/threshold_stage_2.elf` retain loader
symbols and source lines. The runtime emits newline-delimited structured events
over COM1 from the first Rust instruction. Once the kernel fault path is ready,
fatal CPU exceptions include the instruction address needed for offline
symbolization.

For example, a linked address can be resolved without the running VM:

```sh
addr2line -e build/kernel.debug -f -C 0x00100000
```

## boot_omen version 2

The current 88-byte C-layout record contains:

- magic, version, and structure size;
- firmware boot-drive identity;
- feature flags;
- framebuffer address, geometry, bit depth, and RGB masks; and
- memory-map address, count, and entry size.

Flag bit 1 attests that memory enumeration reached its firmware-defined end.
The kernel requires this flag, an exact 24-byte descriptor size, one through 64
records, a table wholly below four GiB, and nonwrapping enabled ranges. It then
copies all descriptors into kernel-owned fixed storage. Only kind 1 with exactly
the enabled volatile attribute may contribute frames; every other enabled kind
blocks overlaps during normalization.

Framebuffer presence is similarly a publication contract rather than a hint.
The kernel independently rejects advertised invalid geometry, extent, color
channels that exceed the declared bit depth or overlap one another,
identity-map bounds, or overlap with low and kernel memory. A safely validated
packed-pixel aperture carries a human-visible memory-failure surface; otherwise
the rejection remains attributable over COM1 and VGA text. This keeps the
machine and human failure projections aligned without trusting an invalid
framebuffer address.

All referenced bootstrap memory must fit inside the first four GiB identity
map. New fields append to the structure, the size field gates access, and
incompatible semantics require another version change. Version 2 is canonical
only for this handoff revision and is not a promise that later revisions retain
its representation.

## Modularity contract

Threshold may gain BIOS, UEFI, arm64 firmware, measured-boot, recovery, and
network-boot frontends. They should converge on the same boot_omen schema and
shared validation rules. Firmware-specific code must not leak into kernel policy.

Planned modules include:

- storage transport;
- artifact discovery;
- signature and measured-boot policy;
- memory and platform discovery;
- display and diagnostic transport;
- recovery selection; and
- architecture transition.

Each module must declare its allocation, firmware, memory, and failure
assumptions. A feature unavailable on one firmware path is represented
explicitly in the omen.

## Current limitations

The BIOS frontend still assumes enhanced-disk services, a truthful firmware
memory map, and sufficient conventional memory for the staged kernel, uses a
fixed preferred VBE mode, and maps only the first four GiB. Its integrity
hashes are not signatures. The
milestone-0 kernel staging ceiling is 384 KiB and its linked memory ceiling is
64 MiB. The complete-map flag detects truncation and structural discovery
failure but does not authenticate firmware claims. NMI remains masked even
though its diagnostic gate and stack now exist;
deterministic NMI injection and nested-fault evidence are still required before
unmasking it. UEFI, allocator-aware placement, pre-kernel transition fault
handling, and verified boot are roadmap work. Direct BIOS E820 fault injection
is not host-executed. Those firmware branches are inspected in loader code;
QEMU and VirtualBox exercise the production initializer, while kernel tests and
emulator gates exercise the fail-closed handoff boundary.
