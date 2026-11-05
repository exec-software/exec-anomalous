---
title: "Repeated Native Editing"
date: 2026-10-04
tags: [antimatter, native-abi, editor, userspace, persistence]
status: qualified
---

# Repeated native editing — 2026-10-04

This increment begins at `0636bf7` after the qualified native durable failure
proof. The immediate blocker was local editing: the established one-entry model
rejected its second changed insert, and saving intentionally retained that undo
entry. Earlier scripts therefore had to undo before making another edit.

The new opt-in repeated opened state retains the immediate predecessor and
reports any displaced undo entry. Earlier types and proof behavior remain
unchanged. The [model contract](../../docs/architecture/native-editor.md) records
retention, atomic rejection, exact pending save identity and the remaining live
input/window integration gate. No stable ABI or disk-format change is made.

The native trace constructs B with three consecutive edits, edits directly to C
while B is pending, acknowledges B without cleaning C, saves C, undoes to B,
edits to D and saves again. Content versions advance from one to four. Same-VM
remount and cold recovery must verify D independently with no recovery answer
in startup data. The existing native durable kernel mechanism is reused.

## Focused and native evidence

The editor suite passed 28 tests, including ten new policy/atomicity cases.
Default, repeated, failure and combined client configurations passed Clippy.
The repeated kernel host suite and repeated/failure/relaunch combinations passed
the focused checks. The full xtask suite passed 256 tests before the final
viewport strengthening; all eight repeated-oracle groups then passed on the
final source. The full matrix also passed all 257 together.

The native proof passed four independent debug-exit boots (initial and cold at
96 MiB and 320 MiB) and two separate host-quit capture boots. Both captures
verify all 9,216 pixels in the published text rectangle. A review caught that
the original 23-character challenge hid D's last edited character at recovery;
the final 21-character `edit-` plus 64-bit random challenge keeps that glyph
visible. A host test verifies changed pixels in that final visible cell for
both zero and all-one entropy. Captures make no full-screen claim.

Initial phase zero independently requires 283 calls, 42 cooperative switches
and 86 arena copies. Same-VM remount and cold recovery each require 143 calls,
16 switches and 52 copies. All ten initial surfaces and every copied document
request/reply match a manual host reference. The first boot uses 41 device
commands (one identify, 15 reads, 13 writes, 12 flushes); cold uses ten (one
identify, five reads, two writes, two flushes). Both checkpoint slots contain D
at version four; sequences five/six become six/seven across cold mount, and
mount epochs advance from two to three. Retained pre-cold media, the boot image
and bytes outside the exact five-sector extent remain unchanged.

The tested native image has SHA-256
`702ccd6bc2d8cdb4f3b2d7168596567e5cc95b81b8adff0dc787cfb4433eb8d6`;
the kernel is
`5cc41bcff2482be1419957130be20648d2869690960284fd791ccedf2543dc0b`;
the client is
`a391d3acd8eb7a05103e8c807f0483ebef9b2f7d8929587ba4ab1e724cfd25a7`.
The client occupies 26,924 of 32,768 RX bytes and 4,288 initialized/RW bytes.
The source-derived schedule, exact binaries, six observed QEMU argument lists,
media snapshots, captures and guarded run logs are retained locally under
an external validation directory (not distributed).

## Exact client review

The emitted client graph, including all panic paths, uses at most 6,000 of its
8,192 stack bytes. There are no unresolved indirect calls, recursion cycles or
dynamic stack adjustments; two nine-entry record-validator tables and 78 local
function graphs close all indirect transfers and stack-height joins. The image
contains no compiler SIMD or privileged instructions. The review separately
checks startup/arena setup, staged undo replacement and no-op restoration,
retained save snapshots, correlated acknowledgement publication and recovery at
version four. It applies to the exact client hash above, not future builds or
a general interactive frontend.

## Exact kernel review

The unchanged durable mechanism's newly linked kernel was reviewed at its exact
hash above. Bootstrap plus one timer uses 35,487 of 65,536 bytes; the gate,
fault and timer paths use 6,463, 1,591 and 1,311 of 16,384 bytes respectively.
All 70 manually resolved indirect sites, 54 register-definition dominance checks,
12 decoded local tables and 226 local function graphs close without unresolved
admitted transfers or stack-height conflicts. The certificate covers admitted
returning execution; fatal panic formatting is explicitly outside that kernel
bound. Seven explicit FX save/restore instructions are present and no compiler
SIMD is emitted. Authority, role/extent checks, copy-before-I/O, successful read
publication, poisoning and complete owner teardown remain consistent.

## Prior-profile binary comparison

The ordinary durable and failure images match their preceding qualified image
hashes exactly; the failure kernel ELF is whole-file identical. Their client
ELF differences are confined to nonloaded DWARF, with loaded bytes, entry,
program headers, symbols and debug-frame tables unchanged. The immediately
preceding ordinary durable kernel ELF was not retained; this comparison binds
the real current kernel to its exactly matching qualified image instead of
claiming equivalence to an older, pre-failure runtime ELF.

The native and ELF replacement clients remain whole-file identical to each
other at `eb7fdb394aea59c2d0fadb4d6649c692253c0898d37d66ad704de9271f0850a7`.
Against the preceding client, 517 executable bytes change because `from_open`
and `is_dirty` exchange order and their references move. All 70 normalized
function instruction/target sequences remain equal. A fresh complete client
review, including panic paths, retains the 3,904/8,192-byte stack bound, closes
six local tables and 1,015 direct transfers, and finds no CPU-profile violation.
This is a real executable delta, not a diagnostic-only exception.

Native/ELF replacement kernel images change by 525/1,559 bytes respectively:
one/three exact copies of those 517 changed client bytes, plus eight valid
Threshold checksum bytes. Every privileged executable byte, other readonly
constant, data byte, entry, program header, BSS extent, symbol and debug-frame
table remains equal. The ELF loader changes only its embedded deployment copy;
its parser instructions and control graph remain equal. The previous kernel
and loader stack certificates therefore remain applicable, paired with the
fresh common-client review. Exact comparison reports and artifacts are retained
in `regression-review/`, `regression-artifacts/` and `relaunch-client-review/`
under the local evidence root above.

## Final qualification

Implementation checkpoint `9d03b46` was committed and pushed after focused model,
client, kernel and host checks, the six-VM native proof and exact binary reviews.
The complete required matrix then passed on the frozen source:

```sh
make check
make smoke
make fault-smoke
make circle-smoke
make control-smoke
make rite-smoke
make scheduler-smoke
make supervision-smoke
make shell-vessel-smoke
make memory-smoke
make interrupt-smoke
make input-wait-smoke
make shell-smoke
make semantic-surface-transport-smoke
make semantic-presentation-smoke
make normal-exclusion
```

The five additional regressions also passed: normal durable save/recovery,
seven-case durable failures, native replacement, ELF replacement and workspace.
The final matrix therefore passed all 21 jobs, including all 257 xtask host
tests. Six final release jobs passed: a repeated-image rebuild, all-feature
kernel Clippy, normal/workspace image restoration, isolated VirtualBox workspace
boot, and independent lifecycle/pixel/cgroup verification. The rebuilt native
image, client and kernel match the six-VM artifacts byte-for-byte. Normal and
workspace images retain their preceding hashes, and all four retained regression
images match their final rebuilt files.

The private VirtualBox workspace matched all 786,432 RGB pixels of the QEMU
initial frame, satisfied the eleven-record lifecycle oracle, ran its private
VM/service inside the memory scope and powered off cleanly. The user's existing
VM was not adopted or modified. This checks the workspace desktop; the native
application profile's separate VirtualBox CPU-state qualification is still open.

The first full `make check` reached the xtask lint stage and rejected the
oracle's unnamed tuple type under `clippy::type_complexity`. Naming the host
reference type fixed it; focused xtask Clippy passed. The source freeze was
renewed and the complete matrix restarted. Guest sources and the tested binary
hashes did not change. The failed attempt remains in the local evidence.

All 396 non-Markdown source files match the final frozen manifest
`b8876e1d4b0b51793dbddf2a7feef967946d53088e4db430454e586cb065a164`.
`qualification-summary.json` binds the final checks, exact artifacts, images,
VirtualBox receipt and memory summaries. Reviews are specific to those hashes.

Nine serialized guarded scopes used two build jobs, one test thread, a 4 GiB
soft memory ceiling, 6 GiB hard ceiling and 512 MiB swap ceiling. Across the
successful runs and retained failed lint attempt, peak scope memory was
4,296,925,184 bytes (4.002 GiB), and minimum host available memory
was 16,894,099,456 bytes (15.73 GiB). The first matrix recorded
3,553 soft reclaim events. There were zero hard-limit events, OOM events,
OOM kills or host OOM-kill counter increases; no job was aborted for pressure.

The remaining product gate is physical keyboard focus and a live editor window,
with explicit edit rejection, measured rendering and an actual document-service
connection. This qualification remains a bounded scripted Rust client, not the
full Antimatter frontend, general undo history, unbounded document storage or
host-power-loss/hardware durability qualification.
