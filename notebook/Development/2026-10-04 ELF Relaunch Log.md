# ELF editor replacement — 2026-10-04

## Qualified checkpoint

The bounded native/ELF editor replacement comparison is complete under proposed
[ADR 0059](../../docs/adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md).
All 52 qualification jobs and two development-image restorations passed on
369 unchanged frozen non-Markdown source and configuration files. TCG/Intel
and KVM/AMD each passed at 96 MiB and 320 MiB with separate host-quit captures.
The final captures match each other and the qualified native replacement frame.

Work starts from native checkpoint `b79e9c6`. Implementation checkpoint
`cf2e597` was committed and pushed after eight focused jobs and both backend
preflights passed. The complete regression matrix and emitted reviews then
qualified that same frozen source. No failed build, test or VM attempt occurred
in this increment; source-review corrections preceded the first checks.

This is the same bounded Rust editor/service fixture, not a full Antimatter or
Electron port. ELF selects executable spans while execution and services retain
the native `int 0xf4` personality. The next bounded document gate is scoped
filesystem persistence with explicit durability and failure semantics. Ny, HAG,
Windows/macOS personalities and Displacer remain separate work.

## Observed recovery and loading contract

Each scenario runs through native spans and then guest-parsed ELF spans. One
RAM document service survives an editor's retirement and replacement within
each run; each route/scenario starts a distinct store. The old editor saves B
(`const after = 1;`) at version two, then edits C (`const later = 1;`).
Replacement after a clean unsaved exit or a fault before the queued save reaches
the service recovers B/version two. Replacement after C commits but its reply
is lost recovers C/version three. Every successor rejects the old handle, saves
D (`const newer = 1;`), reads, closes and reopens it under fresh authority.

The unchanged 128-byte `ANDS` descriptor, native startup/gate, document protocol
and client executable are preserved. Two ring-3 loader tasks parse the compact
ELF and offer a copied 96-byte inert `ANEL` plan. The controller checks role,
ownership and equality with the independently inspected immutable manifest.
It does not parse ELF or interpret document operations. Negative role/copy/
encoding/geometry/duplicate probes precede valid admission. Both loader owners
and all fifty frames/ten leases retire before application allocation.

The host retains the complete client ELF. A profile-specific compactor elides
only 4,096 proven-zero writable file bytes: a 128-byte prefix still describes a
4,224-byte writable memory extent. Full reconstructed virtual bytes, permissions
and native RX are checked independently. Deployment is 28,800 bytes; loader RX
is 32,240 bytes, leaving 528 bytes within the unchanged 32 KiB limit. The parser,
generic compactor, plan encoding and mapping bounds remain unchanged.

The selected route's RX/RW spans populate initial owners and every replacement
attempt, including injected population failure, committed-but-unadmitted
cancellation and the final successor. Whole private regions are checked before
startup overlay. The service remains the exact current task across unpublished
bootstrap maintenance, with full private-region hashes and exact identity,
root, stack, saved frame and CPU comparisons at eight checkpoints. Only userspace
changes document authority after validating successor metadata. Both yield and
exit reject before the exact-generation acknowledgement.

Live pair accounting follows 66 frames/twelve leases, 33/six for the retained
service, then 66/twelve after replacement. Partial-clear and construction failure
paths retain ownership until retry or cancellation. Final teardown restores all
resources and revokes route, task and circle authority. Saved document state
does not survive service destruction or machine restart; this is not filesystem
durability or a general process manager.

## Independent evidence and emitted bounds

Each complete transcript contains 222 ordered records, including 48 surface
checkpoints and 138 manually constructed document packet hashes. Both routes
independently pass the qualified native known-byte oracle. Calls are 292/293/303,
switches 47/47/49 and arena copies 99/100/104 for the three cases on each route.
All 99 per-route lifecycle, packet and surface records also match the previous
native qualification after removing only container labels and individually
bounded timer ticks. Whole rendered captures match exactly.

Each complete proof executes eighteen document task lifetimes: six retained
services, six original editors and six replacements. Six committed but unstarted candidates
are cancelled separately. Four editor faults are contained; 589,824 arena bytes
are verified freshly zeroed. The two loader tasks and four prior native-baseline
task lifetimes are separate from those totals.

The exact kernel ELF SHA-256 is
`0125eaddac667acf5f4e97661d733a880f725185121d16e8bd0a2bfc8f26b38d`.
The client ELF SHA-256 is
`6e550599d6d722aca9dc024cf1e49bfea682393f2a02b5ba1c3555a86a6a2546`;
the complete file is byte-identical to the preceding native certificate.
The loader ELF SHA-256 is
`28bcff5609cd974346ed6fd6cf6c84cfc2efd70ae16913ce6e3645e27a942582`.
The qualified image SHA-256 is
`5bec05b6521ab064e13826049dd2c8cd15bdf0aa35bf3cb4b2fc4f761bc2bf40`.

The client retains its complete 3,904/8,192-byte stack bound including all
compiled panic paths. Loader executable text is identical to the preceding
ELF-document loader; its new read-only deployment is independently verified.
The loader closes all 148 direct control transfers, with no indirect transfers,
recursion or dynamic stack adjustments, at 912/8,192 bytes including panic paths.
Its embedded ELF occupies a read-only data section inside an executable mapping;
all inspected loader transfers remain inside loader text.

Kernel bootstrap use is bounded at 31,672/65,536 bytes, or 32,983 with one timer.
The loader/baseline envelope is 30,952 and retained-service maintenance 29,016.
Gate/fault/timer bounds are 5,943/1,591/1,311 within each 16 KiB transition stack.
All 68 manual sites and twelve local jump tables are closed, with no admitted
cycles or unresolved indirect targets. Fatal kernel panic/Debug formatting is
excluded from those admitted-path bounds; nonlocal entry/checkpoint/return are
reviewed separately. The exact critical-entry review found no remaining blocker.

`make check` passed all host suites, feature builds, artifact checks and lint.
Host counts include 491 kernel tests for each replacement profile, 531 with all
kernel features, 221 tooling tests and eighteen each for the editor and document
protocol. The actual host Antimatter document reference matched, with its source
hashes and corresponding files retained.

Fifteen prior payload blobs and the earlier ELF-document deployment/loader/plan
remain unchanged. All nine previous capture pairs remain identical. Separate
hash-bound baseline reports explain any metadata-only image changes and verify
relevant emitted code/stack equivalence. Both ordinary and workspace development
images exactly match the preceding qualified images. VirtualBox 7.2.6 is installed,
but this account cannot access its root-owned `0600` driver device; no VirtualBox
run is claimed.

## Memory and retained records

The parent serialized all compiler and QEMU jobs with two compiler workers,
one test thread and `make -j1`. Limits were `MemoryHigh=4G`, `MemoryMax=6G`
and `MemorySwapMax=512M`. Two-second monitoring would stop work for a new OOM
or host available memory below 4 GiB. Peak scope usage was
4,297,015,296 bytes; minimum host available memory was
21,049,294,848 bytes. There were 4,675
soft-limit events, zero hard-limit events and zero OOM events. Peak sampled
scope swap use was 30,081,024 bytes. Host-wide swap
totals include other workloads and are recorded separately.

Evidence lives under
an external validation directory (not distributed): frozen source,
preflight and final artifacts, transcripts, captures, memory samples,
`final-receipt.json`, emitted reviews, `route-comparison.json`, baseline reports,
reference source, restored image hashes and final Git receipt. `evidence.sha256`
binds every retained file after final commit/push verification.

Commands are `make elf-relaunch-inspect`, `make elf-relaunch-image` and
`make elf-relaunch-smoke`.
