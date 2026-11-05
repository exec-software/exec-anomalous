# ADR 0057: Prove ELF loading of the native document contract

- Status: Proposed; bounded ELF/native document comparison qualified
- Date: 2026-10-04
- Related: ADR 0045, ADR 0051, ADR 0055, ADR 0056

## Context

[ADR 0056](0056-prove-task-bound-native-document-open-and-save.md) qualifies
bounded document open/read/save/close, exact snapshot acknowledgements and
uncertainty after a lost reply. The selected next comparison runs that same
contract through the independently executing ELF loader from
[ADR 0051](0051-prove-userspace-elf-loading-against-native.md). ELF is the
executable container; both routes retain the native `int 0xf4` personality.

The unchanged document client fits the existing eight-page RX and two-page RW
application bounds. Its full linked ELF contains 4,224 zero-valued RW file
bytes. Preserving that file span in the compact deployment produces a
32,896-byte ELF, which alone exceeds the loader's 32 KiB RX allowance. The
existing ELF subset already permits zero-filled memory beyond RW file bytes.
Use that established representation without expanding executable admission.

## Decision

Add an opt-in `elf_document_test` proof. Retain the full linked document client
and its exact RX bytes unchanged. A new document-specific host compactor first
verifies the existing all-zero RW contract, then retains a 128-byte zero RW file
prefix and the original 4,224-byte RW memory extent. It elides exactly 4,096
verified-zero file bytes. Reconstruct the complete virtual RW image from its
file prefix and zero tail and compare it with the independently inspected full
client before accepting the deployment. This preserves initialized virtual
bytes and permissions, not the full client's original RW file geometry.

The existing generic compactor retains its exact file-byte preservation rule.
The safe `anomalous-elf-adapter` parser, sectionless ELF64 subset, two PT_LOADs,
fixed entry and virtual addresses, 64 KiB deployment limit and canonical
96-byte `ANEL` version-one plan remain unchanged. No extra segment describes an
arena, stack, startup descriptor, service, task identity or authority. No
interpreter, relocations, dynamic linking, TLS or foreign syscall policy is
introduced.

The bounded prototype produces a 28,800-byte deployment and a 32,240-byte
loader RX image, leaving 528 bytes inside the unchanged 32,768-byte limit.
Those artifact-fit measurements do not alone establish runtime qualification.
Linker and inspection bounds remain mandatory; later growth past the limit
requires a separate design decision rather than automatic widening. The
preceding loader and application profiles retain their old acceptance rules.

Two independently owned ring-3 loader tasks receive the full immutable
28,800-byte deployment slice. They parse its ELF headers and program-header
table and validate extents and padding; payload bytes remain opaque. Only the
controller-designated loader role can offer one copied plan. The kernel checks
current ownership, exact wire encoding and equality with the independently
inspected immutable manifest. It does not parse ELF headers. Wrong role, bad
copy, short/corrupt records, altered geometry
and duplicate admission fail without publishing application tasks. A valid
offer after the negative probes must still succeed. Both loader tasks, roots,
contexts, CPU state and leases retire before any document pair is allocated;
only the admitted inert plan survives this transition.

Run all four document scenarios through native flat spans and accepted
ELF-derived RX/RW spans. Native and ELF runs for each scenario are adjacent.
The native route retains the original RW file span; the ELF route supplies the
128-byte prefix and depends on verified zero fill for the omitted bytes. Both
routes check the complete private RW pages, stack and arenas before installing
the trusted 128-byte `ANDS` descriptor at `0x801000`. Its storage lies in the
ELF route's zero-filled RW tail. The descriptor is created from retained task
and channel identities; it cannot create authority.

Every document pair reuses the four-region native profile: eight RX code pages,
two RW data pages, eight guarded RW/NX arena pages and two guarded stack pages
per task. The pair owns 66 frames (48 data and 18 page-table frames) in twelve
leases. The loader pair keeps the earlier three-region, 50-frame, ten-lease
profile. Loader and document pairs run sequentially. All cleanup, eager CPU
ownership and retained-storage obligations remain explicit.

## Comparison and acceptance

Each route must independently pass the native document oracle. Expected
document wire bytes are constructed from explicit checkpoints without invoking
the service's implementation. Equal outputs from two incorrect routes are
insufficient. Require exact open/read/save/close and rejection packet hashes,
semantic surface hashes, counters, saved-byte transitions, fault kind,
uncertain pending snapshots, resource restoration and authority retirement.
Only bounded timer ticks are excluded from route equality. The two normal
documents must remain distinct.

Before-commit and after-commit faults retain their distinct inspected UD2
sites and real volatile store observations. Each leaves a delivered save
without a semantic reply; the editor keeps the outcome unknown and dirty.
Close/reopen preserves committed bytes only while the same service survives.
The final fresh pair starts another RAM store and zeroed private arenas.

Compare every rendered destination byte between adjacent normal routes before
publishing the ELF frame, preserve pixels outside the editor patch and retain
whole-frame host captures. Pixel expectations share the qualified renderer;
this is not an independent font implementation. The actual Antimatter host
save-store reference remains a separate semantic control.

The strict transcript contains 224 ordered records: the unchanged 21-record
native baseline, readiness and loader receipts, eight route/scenario receipts,
44 surface hashes, 148 document packet hashes and final success. Reject missing,
duplicate, reordered, extra or mutated fields and records. Require exact route
labels and timer bounds; no filtering away unexpected proof records is allowed.

Host artifact tests must reject nonzero elided bytes, changed RX or virtual
geometry, failed reconstructed-image equality, malformed deployment records,
undeclared wrapper bytes, stale aliases and exceeded loader bounds. Retain the
full client ELF, compact deployment, loader ELF, immutable plan, disassembly
and hashes. Recheck emitted stack bounds, indirect targets, CPU instructions,
startup and fault entry for the exact final kernel and loader binaries.

Qualify TCG/Intel and actual KVM/AMD at 96 MiB and 320 MiB with separate captures,
all required repository checks, prior ABI/runtime/document regressions and
restored development images. Preserve the preceding native client and qualified
payload bytes. The new profile stays excluded from ordinary images. Builds and
QEMU runs remain serialized under recorded memory/OOM limits. Record final
counts, hashes and peak memory only after qualification completes.

## Bounded qualification

All 48 qualification checks and two development-image restorations passed.
The native and ELF routes each passed the document wire oracle and compared
final pixels under TCG/Intel and KVM/AMD at 96 MiB and 320 MiB, with retained
captures. Exact final artifact and emitted-stack reviews accompany the runtime
evidence in the [ELF Document Log](../../notebook/Development/2026-10-04%20ELF%20Document%20Log.md).
The qualification is for the fixed inspected fixture and native personality
described here; this ADR remains Proposed and does not stabilize an ABI.

## Limits and follow-up

This qualifies an inspected fixed ELF fixture using the existing native
execution and service personality. It does not admit arbitrary ELF files,
establish Linux compatibility, stabilize a loader ABI or port Antimatter/
Electron. Kernel code retains mechanisms; document and save policy stay in
userspace. Proof-scoped document identities and scenario values are not a
global namespace. Native and ELF runs are isolated comparisons, not concurrent
clients or cross-service persistence.

The subsequent document gate retains a service across editor owner replacement,
then introduces scoped filesystem persistence with explicit durability
semantics. Ny adoption, HAG, Windows and macOS personalities remain separate
qualification work.
