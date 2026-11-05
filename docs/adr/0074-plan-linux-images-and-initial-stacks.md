# ADR 0074: Plan Linux images and initial stacks without execution authority

- Status: Accepted; host planning only, guest execution remains open
- Date: 2026-10-07
- Related: ADR 0008, ADR 0051, ADR 0052, ADR 0053, ADR 0061, ADR 0073

## Context

ADR 0073 selects the existing Linux Antimatter/Electron application. The current
ANEL loader admits a fixed two-segment, sectionless ELF with native fixture
addresses. The installed unmodified Linux `true` already requires ET_DYN/PIE,
four LOAD segments, an interpreter and a Linux initial process stack. libc and
Electron add TLS, dynamic metadata and further runtime requirements. Widening
the old parser would silently change the qualified fixture contract.

Before changing privileged mapping and entry, establish separately testable
userspace descriptions of actual image bytes and initial stack contents. A
description must not itself authorize mapping, interpret GNU properties as
accepted CPU requirements, or imply the dynamic linker has executed.

## Decision

1. Add opt-in `linux_image` and `linux_startup` modules to the existing portable
   ELF adapter behind its default-off `linux_image` Cargo feature. Both are
   allocation-free and retain the crate's unsafe-code prohibition. Keep the old
   parser and 96-byte ANEL plan unchanged. This is an internal experimental API,
   not a new serialized ABI or privileged admission interface.
2. Parse immutable borrowed ELF64 little-endian x86-64 ET_EXEC/ET_DYN bytes.
   Validate file extents, header/section-table geometry, LOAD alignment and
   ordering, nonoverlapping rounded pages, readable R/RX/RW permissions without
   W+X, and caller-supplied resource budgets. Extended numbering and unsupported
   mapping semantics fail explicitly. A plan validates the selected bias and
   lower-canonical address geometry; ET_EXEC requires bias zero. Executable
   entry must lie in initialized executable bytes. A library role does not
   select an entry point.
3. Expose exact initialized, BSS, rounded mapping, file-copy and zero-fill
   extents. For a nonaligned initialized LOAD, copy the leading file-page prefix
   and zero the remaining page slack. Do not expose trailing file bytes beyond
   the segment's declared initialized end. Derive PHDR only when the entire
   table belongs to one initialized LOAD; validate any explicit PT_PHDR against
   that derivation. There is no fabricated AT_PHDR when headers are not loaded.
4. Preserve raw program headers and borrowed interpreter-path bytes. A path is
   absolute and NUL-terminated in the source; no global length limit, Unicode
   requirement or lexical normalization is introduced. Source/image budgets are
   separate from namespace policy. Dynamic, TLS, RELRO, stack, note and GNU
   property metadata remains visible. No relocation, TLS installation, RELRO
   protection, CPU qualification or dependency resolution occurs here.
5. Build a Linux x86-64 initial stack only from caller-supplied facts. It contains
   argc, argv/NULL, envp/NULL, auxiliary pairs/AT_NULL, copied strings and sixteen
   caller-supplied random bytes, with a 16-byte-aligned initial RSP and no return
   slot. Core entries describe the main PHDR/PHENT/PHNUM, 4096-byte pages, main
   entry, interpreter bias, random bytes and exec filename. Typed extra facts
   are explicit: credentials, secure mode, clock, CPU capabilities, platform
   pointers and vDSO are never invented. Reject duplicates, overrides, interior
   NULs, invalid addresses, arithmetic overflow and insufficient space before
   any write. The output extent is the budget, not a global path limit.
6. The host inspector binds reports to the exact held source bytes with SHA-256,
   checks cross-image and stack mapping disjointness, and writes only into a new
   evidence directory after semantic validation. The explicitly supplied
   interpreter is reported alongside the image's declared interpreter path;
   this does not authenticate it or resolve a guest namespace. Initial RIP is
   the interpreter entry, AT_ENTRY remains the main entry, and initial RDX is
   zero. The review profile has one argv item, an empty environment and only
   core auxv entries; it is not a complete runnable Linux environment.

## Host review profile and commands

The host tools budget each image at 512 MiB, 128 program headers, 32 LOADs and
131072 mapped pages. These explicit operation budgets can evolve separately
from the portable caller-supplied limits. The composed review uses main PIE
bias `0x40000000`, interpreter PIE bias `0x60000000`, and a 64 KiB stack at
`0x7fff00000000`. These are inspection choices, not an ASLR or address-allocation
policy. The tool does not create a filesystem view or copy dependencies into
the guest.

```sh
cargo xtask linux-image-inspect /usr/bin/true 0x40000000 executable
cargo xtask linux-image-inspect /lib/x86_64-linux-gnu/libc.so.6 0x70000000 shared-object
cargo xtask linux-process-inspect /usr/bin/true /lib64/ld-linux-x86-64.so.2 RANDOM_FILE NEW_DIRECTORY
```

`RANDOM_FILE` must be a regular file containing exactly sixteen caller-supplied
bytes. Reports carry exact source path bytes, hashes, all raw program headers,
mapping/copy/zero ranges and explicit limitations. The process command writes
`stack.bin` and `report.json`; existing output is never overwritten. The tools
need `sha256sum` and never execute the input images.

`tools/verify_linux_process.py` independently decodes real source headers,
materializes proposed copy/zero operations and decodes the output stack. It also
retains conventional `readelf` output. Run it under the same memory guard as
builds, supplying local binaries explicitly:

```sh
python3 tools/verify_linux_process.py \
  --xtask /ABSOLUTE/REPOSITORY/target/debug/xtask \
  --output /NEW/EVIDENCE/DIRECTORY \
  --program /usr/bin/true \
  --interpreter /lib64/ld-linux-x86-64.so.2 \
  --library /lib/x86_64-linux-gnu/libc.so.6 \
  --electron /ABSOLUTE/ANTIMATTER/node_modules/electron/dist/electron
```

## Qualification and next execution boundary

Malformed-image and failure-atomic stack tests, host and freestanding-target
linting, actual-artifact inspection and the required native regression suite
qualify this host slice. The dated notebook records the exact artifact hashes,
commands, outcomes and memory observations. A successful parse or stack oracle
is not successful execution of `true`, libc, Electron or Antimatter.

The next privileged work must accept owned multi-region plans without moving
ELF interpretation into the kernel. Current fixed code/data geometry and the
eight-region capacity cannot represent main plus interpreter plus stack.
Checked entry/return needs an owned executable-interval set rather than a single
interval widened across holes. TLS ownership must cover every syscall, interrupt,
return, switch and retirement path; merely writing FS base for ARCH_SET_FS is
insufficient. Linux filesystem/descriptor, mapping/protection and loader-startup
services remain separate userspace-veil work. Let the unchanged dynamic linker
perform its relocations and library loading once those mechanisms exist.

## References

- [System V gABI program headers](https://gabi.xinuos.com/elf/07-pheader.html)
- [x86-64 psABI initial process state](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/low-level-sys-info.tex)
- [Linux 6.17 ELF loading and stack construction](https://github.com/torvalds/linux/blob/v6.17/fs/binfmt_elf.c)
- [glibc x86-64 TLS initialization](https://github.com/bminor/glibc/blob/master/sysdeps/x86_64/nptl/tls.h)
