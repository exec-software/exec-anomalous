---
title: "Memory Ownership Log — 2026-09-09"
date: 2026-09-09
tags: [development, kernel, memory, paging, ownership, qemu, virtualbox]
status: complete
domain: foundations
---

# Memory Ownership Log — 2026-09-09

## Session outcome

Establish the smallest trustworthy bridge from Threshold's firmware memory map
to allocator-selected physical frames and owned virtual mappings. This work
followed the fixed circle proof without promoting that experiment's static
storage, gate, address, or recovery path into ABI. ADR 0012 is accepted after
host, QEMU, VirtualBox, exclusion, and regression evidence passed.

## Recovery observation

Development resumed after a local power interruption. `git fsck` reported no
damage; local `main` and `origin/main` both resolved to the accepted circle
checkpoint `37fb0b7`. The dedicated `Anomalous Dev [Threshold]` VirtualBox VM
was running again with the normal image, and its serial log still contained
`boot.ready` and `bootstrap_shell.ready`.

This observation confirmed repository recovery only. The memory-specific
evidence below was subsequently produced from the hardened implementation.

## Accepted boundary

[ADR 0012](../../docs/adr/0012-prove-bounded-bootstrap-memory-ownership.md)
accepts a bootstrap-processor-only manager for 4 KiB frames below four GiB. It
requires a complete version-2 handoff, kernel-owned descriptor copy,
order-independent reserved-wins normalization, conservative low/kernel/display
reservations, opaque allocator- and generation-checked frame leases, managed
non-W+X virtual bindings, CR3 withdrawal before release, and verified
zero-before-reuse.

The first discovery audit found a blocking flaw: Threshold had stopped E820
enumeration on firmware error or after 64 entries without reporting whether a
continuation remained. No frame allocator could safely consume that table until
the producer guaranteed complete enumeration and the kernel required it.
Threshold now publishes version 2's completeness bit only at the
firmware-defined end; every error and over-capacity continuation fails before
kernel entry, and the kernel independently requires the bit.

Subsequent independent reviews found five more authority or observability gaps:

- a lease did not identify the allocator instance that issued it;
- safe clearing callbacks could name arbitrary physical memory;
- a caller retained a releasable data lease after mapping;
- an advertised invalid framebuffer could evade reservation or hide a failure;
  and
- the transcript validator did not require all boot prerequisites or reject
  every error-level record.

The accepted implementation adds nonwrapping allocator provenance, makes raw
clear/allocation/release authority explicitly unsafe, transfers the noncopyable
lease into the mapping owner, parses and reserves framebuffers fail-closed,
draws memory failures through only a safely validated framebuffer, requires VBE
2.0 or newer and selects its version-appropriate legacy or linear pitch/RGB
fields, requires supported graphics and linear-framebuffer mode attributes
before Threshold activates or publishes them, independently checks RGB
bit-depth bounds and non-overlap in Rust, and makes the strict harness require
ordered prerequisites plus an error- and fatal-free lifecycle.

## Acceptance matrix

| Check | Required observation | Status |
| --- | --- | --- |
| Handoff | Incomplete enumeration cannot reach memory initialization | Passed; loader branches reviewed and kernel/emulator fail closed |
| Normalization | Host vectors prove checked, aligned, reserved-wins disjoint frames | Passed in `make check` |
| Ownership | Foreign, stale, altered, duplicate, mapped, overlap, active-root, and W+X operations fail without partial mutation | Passed in host tests and explicit feature records |
| Memory smoke | Two QEMU RAM sizes complete allocate/map/switch/withdraw/clear/reuse | Passed at 96 MiB and 320 MiB |
| Exclusion | Normal image contains no memory-test entry or terminal event | Passed |
| Regressions | Existing check, boot, fault, circle, and shell gates remain green | Passed |
| VirtualBox | Full feature trace, followed by restored normal shell | Passed at 512 MiB; normal VM restored and running |

## QEMU evidence

`make memory-smoke` completed at both configured RAM sizes. The strict parser
required `boot.omen.accepted`, `boot.memory.map`, `kernel.memory.ready`, and
`boot.ready` in order before exactly seven feature-test lifecycle events.
Together with `kernel.memory.ready`, those form the eight ownership evidence
records validated in detail. The parser rejected any error or fatal record and
required the final counts to equal the initial counts.

| RAM | Free frames | Unavailable frames | Data frames | Root frames |
| --- | ---: | ---: | --- | --- |
| 96 MiB | 24,162 | 1,024,414 | `0x17d000`, `0x17e000` | `0x17f000`, `0x183000` |
| 320 MiB | 81,506 | 967,070 | `0x17d000`, `0x17e000` | `0x17f000`, `0x183000` |

The 57,344 additional free frames equal exactly 224 MiB, matching the emulated
RAM difference. Both traces carried the distinct A and B markers through the
A-to-B-to-A sequence and explicitly reported `overlap_rejected=true`,
`duplicate_binding_rejected=true`, and `active_root_rejected=true`. After CR3
restoration and withdrawal, both data extents were reacquired at the same
addresses and observed zero; final free, data-owned, and table-owned counts
matched their initial state.

## VirtualBox and normal-image evidence

The dedicated `Anomalous Dev [Threshold]` VM ran the memory feature at 512 MiB.
Its complete trace reported eight normalized regions, 130,675 free and 917,901
unavailable frames, the same selected data and root extents, the same marker
sequence and negative checks, full free-count restoration, and no error or fatal
record. The captured evidence is preserved as the ignored local artifact
`build/virtualbox-memory.log`.

The normal image was then rebuilt and restored to the VM. At that checkpoint it
reported 130,686 free frames, emitted `boot.ready` and
`bootstrap_shell.ready`, and was observed running. Normal-image exclusion also
proved that the memory-test entry path, events, symbols, and marker residues were
absent.

## Evidence boundary

Direct BIOS E820 fault injection cannot be executed faithfully as a host unit
test, and the singleton allocator publication path is intentionally a one-time
global transition rather than resettable test state. Those paths are covered by
code review. QEMU and VirtualBox execute the production initializer, while
kernel tests and emulator gates exercise its fail-closed handoff boundary. The
evidence assumes BIOS and hardware describe memory truthfully after all
structural checks.

## Explicit limits

This is not a heap, production page-fault policy, general address-space API,
general process-isolation claim, circle, vessel, task, scheduler, shared-memory
design, DMA contract, or stable ABI. The existing first-four-GiB identity map
remains supervisor writable and executable, and the proof is BSP-only with
interrupts disabled. Firmware truthfulness, high memory, concurrency, DMA, and
general reclamation remain outside this accepted slice.
