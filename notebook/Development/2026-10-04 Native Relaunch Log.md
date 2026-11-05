# Native editor replacement — 2026-10-04

## Qualified checkpoint

The bounded native editor replacement slice is complete under proposed
[ADR 0058](../../docs/adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md).
All 50 qualification jobs and both development-image restorations passed on
366 unchanged frozen non-Markdown source and configuration files. TCG/Intel and KVM/AMD each
passed at 96 MiB and 320 MiB, followed by a separate host-quit capture. The
captures and qualified artifacts match across backends.

The checkpoint sequence is `9e00128` (authenticated userspace reassignment),
`a1247c3` (separate native client) and `1c89935` (integrated replacement proof,
including first-boot corrections). Each was pushed. This qualification records
the final frozen integration source, not the superseded first client artifact.

This remains a bounded Rust editor/service fixture. Committed bytes survive
only while the same RAM service lives; there is no filesystem durability,
service restart survival, general process manager or full Antimatter/Electron.
The next comparison should repeat retained-service replacement through the
userspace ELF loader against this native baseline, before scoped filesystem
persistence with explicit durability rules. Ny, HAG and the Displacer integration
remain separate work.

## Observed replacement contract

Each original editor opens A (`const before = 1;`), edits and acknowledges saved
B (`const after = 1;`) at service version two, then edits C (`const later = 1;`).
Because the current editor has one undo slot, the command stream explicitly
undoes to A and replaces its selected word with C. C has buffer revision four;
the saved baseline remains B until another save is committed.

The three replacement cases are an orderly exit with C unsaved, an editor fault
while C's save remains queued before service delivery, and an editor fault after
the service commits C/version three but before the editor consumes its reply.
The first two successors open B/version two. The third opens C/version three.
A fresh open reconciles actual surviving state without acknowledging a lost
request to the destroyed editor.

Every successor rejects an old handle, saves D (`const newer = 1;`) under its
own handle/request sequence, reads it, closes and reopens under another handle
generation, and checks the exact bytes again. Four copied routes carry surfaces,
input, document requests and replies. The unchanged `ANDS` descriptor and
native `int 0xf4` personality remain within the existing four-region bounds.

The service retains its identity, root, user/transition stacks, private data,
arena, exact saved frame and CPU image across bootstrap maintenance. The
runtime is unpublished during that interval. Eight service checks per case
cover complete private-region hashes and exact frame/CPU/identity comparisons
before successor metadata enters service memory. Userspace validates that
metadata and changes the store authority; a generation-specific acknowledgement
gates replacement execution. Both yield and exit reject before acknowledgement.
Old route values, retired identities and stale/replayed acknowledgements reject.

Each case exercises a retained transition-stack clear failure and retry, a
preparation failure and cancellation, and cancellation of a committed but
unadmitted candidate after publication preflight rejection. The latter is not
rollback after scheduler admission. Accounting follows 66 frames/twelve leases
for the pair, 33/six for the retained service, and 66/twelve for the rebuilt pair.
Partial cleanup retains its remaining owner until retry. Final teardown restores
all frames/leases and revokes all authority. Cancelled task generations may
create gaps; document handle generations remain independently validated.

## Evidence and artifact bounds

Every completed transcript contains 122 ordered records: the prior 21-record
native baseline, lifecycle evidence, 24 independently predicted surfaces and
69 manually constructed document packet hashes. Calls are 292/293/303,
cooperative switches 47/47/49 and arena copies 99/100/104. Each case starts three
executed tasks, observes one cancelled unstarted candidate and verifies 98,304
zeroed arena bytes; the three cases total 294,912 bytes. Two editor faults are
contained. All three final frames render reopened D identically.

The final client contains 23,334 initialized RX bytes, 4,224 initialized RW bytes,
and the unchanged 128-byte descriptor at `0x801000`. Its ELF SHA-256 is
`6e550599d6d722aca9dc024cf1e49bfea682393f2a02b5ba1c3555a86a6a2546`.
The final kernel ELF SHA-256 is
`6903139bc55ba9ab43b0e09910d4693f80eb751aa86e85cdaffa96ee472a7e27`.
The qualified image SHA-256 is
`a95b68df8e31b41945a417a61c38d080d7a64a02ee51598097aa6f3c11b5bc04`.

The hash-bound emitted review closes all admitted control transfers. The client
complete call-chain bound, including every compiled panic path, is 3,904/8,192
bytes. Its six local jump tables are bounded; there are no unresolved indirect
calls, recursion or dynamic stack adjustments. The kernel bootstrap bound is
28,744/65,536 bytes, or 30,055 with one timer; the maintenance path is 26,088.
Gate/fault/timer paths use 5,927/1,591/1,311 bytes of each 16 KiB transition stack.
All 67 manual sites and twelve local branches are closed. Kernel bounds cover
admitted paths; fatal panic/Debug formatting is excluded. Nonlocal entry,
checkpoint and terminal return are reviewed separately without adding them as
recursive calls. The separate critical-entry review verifies retained CPU state,
unpublication, service-first return and the acknowledgement barrier.

`make check` passed all feature combinations, builds, artifact checks and lint.
Host results include 491 native-relaunch kernel tests, 531 all-feature kernel
tests, 207 xtask tests, eighteen editor-contract tests and eighteen document
protocol tests. The Antimatter document reference matched, with its source
hashes and relevant files retained. All preceding matrix jobs passed.

Fourteen earlier embedded artifact blobs and the ELF document deployment/loader/plan
remain byte-identical. All eight earlier capture pairs remain unchanged.
Native/ELF document kernel executable bytes and stack bounds are unchanged;
17/22 raw image bytes changed only through decoded panic-line metadata and
valid Threshold checksums. Their full client ELF changed only in debug data;
all allocated sections, load segments, symbols and disassembly are unchanged.
The normal and workspace images are exactly the previous qualified images.
VirtualBox 7.2.6 is installed, but `/dev/vboxdrv` is root-owned with mode
`0600`; this account has no read/write access. No VirtualBox run is claimed.

## Corrections retained during qualification

The first client inspection rejected a missing linker build script. The script
was added and inspection passed. The first native boot then exposed the
single-undo-slot choreography error described above; an unchanged-image GDB
hardware breakpoint localized the service insertion panic. The host regression
now checks the next save's request, handle, base version and revision after the
explicit undo/re-edit stream. Independent boundary review also found that a
service exit could dispatch the successor before acknowledgement; the guard and
live yield/exit rejection probes close that path. Failed attempt receipts and
superseded exact artifacts remain retained alongside the final evidence.

## Memory and retained records

All builds and QEMU runs were serialized by the parent with two compiler jobs,
one test thread and `make -j1`. Limits remained `MemoryHigh=4G`, `MemoryMax=6G`
and `MemorySwapMax=512M`. Two-second monitoring would stop a job for a new OOM
or host available memory below 4 GiB. Across all retained scopes, peak usage was
4,296,871,936 bytes and minimum host available memory was
16,796,483,584 bytes. There were 4,398
soft-limit events, zero hard-limit events and zero OOM events. Host-wide swap
totals include other workloads and are not a measurement of this scope's swap use.

Evidence lives under
an external validation directory (not distributed):
`final-receipt.json`, the frozen source, exact artifacts, strict transcripts,
captures, independent call model, stack/control reports, baseline comparison,
failed attempts, memory samples, image restorations and final Git receipt.
`evidence.sha256` binds the retained files after final commit/push verification.

Commands are `make native-relaunch-inspect`, `make native-relaunch-image` and
`make native-relaunch-smoke`.
