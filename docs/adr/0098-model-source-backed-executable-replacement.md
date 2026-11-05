# ADR 0098: Model source-backed executable replacement

- Status: Proposed; implementation and host qualification in progress
- Date: 2026-10-19
- Builds on: ADRs 0090, 0096 and 0097

## Context

The unchanged Linux interpreter's observed sequence 12 is
`mmap(0x100028000, 0x198000, 5, 0x812, 3, 0x28000)`. It requests a fixed private
read/execute replacement over pages 40 through 447 of the retained libc mapping.
The complete requested range lies inside the source-backed prefix. Its source
offset corresponds exactly to its offset within the original mapping.

The existing graft derives every expected candidate entry from one immutable
read-only, non-executable mapping plan. Its withdrawal operation ends that
graft's lifecycle. Neither clearing NX in isolation nor splitting the plan's
regions preserves the existing ownership, copy and cleanup contract.

## Decision

First qualify an inert request contract and a host-only permission transaction
model. Leave the live sequence 12 request unanswered. This checkpoint adds no
native gate operation, userspace call, page-table mutation or executable mapping.
Its experimental request words are not a stabilized public ABI.

Keep Linux syscall, flag and descriptor interpretation in userspace. Recognize
the exact observed request against the retained file-service snapshot without
advancing its offset, watermark or fence. Decode native metadata separately:
fresh invocation sequence, retained mapping incarnation, complete source binding,
aligned contained range, explicit matching source offset, read/execute rights,
and zero reserved fields. Executable coverage must fit actual source bytes;
zero padding and reserved virtual tail cannot become executable. Numeric handles,
hashes and successful decoding confer no authority over a live owner.

The native host model borrows the retained original mapping owners. Preserve
their plans, region counts and lease geometry. Reject cross-owner table/data
lease overlap even when both owners have internally valid geometry. Actual
allocator lineage remains an external obligation. Derive a single contiguous
permission overlay from those original mappings: only NX can change on the
selected read-only user leaves. Neighbours, physical addresses and private
ancestry remain exact. Hardware Accessed allowances apply only to admitted
graft-visible entries; Dirty and user writable/executable combinations remain
forbidden.

Model reconfiguration separately from terminal withdrawal. Before any modeled
leaf mutation, require complete old-tree verification, revoked selection,
detachment and completed invalidation. Record attempted stores before observing
their outcome, including failures reported after a write. Uncertain progress
bars resume and ordinary release. Complete candidate verification, publication
and a separate invalidation precede modeled readiness. Invocation correlation and mutation-epoch equality bind transitions within
one model instance. Globally fresh epochs, a sealed scheduler invocation and
authentication/preservation of the complete saved register frame remain
obligations of the future adapter. Terminal quiescence, withdrawal and invalidation are distinct
prerequisites for the model's cleanup predicate.

These transitions consume simulated observations. They do not authenticate
kernel scheduler state, execute an invalidation instruction, establish physical
withdrawal or authorize actual lease release. The model is test-only; live
integration must preserve its invariants through actual retained owners.

## Qualification

Exercise the actual observed 408-page geometry, malformed native fields and Linux
register shapes, stale bindings and sequence/epoch values, source-offset and
padding boundaries, altered ancestry/neighbours, and every attempted-store
failure position. Include false-after-write and failed readback observations,
blocked resume/release while uncertain, and terminal cleanup after partial work.
Tests must compare complete simulated trees and retain exact original ownership.

Run protocol and explicit kernel library tests, strict protocol and freestanding
kernel lint, required repository checks and affected mapping/application regressions under the serial OOM guard.
Compare the rebuilt development images with the qualified baseline. Guest runs
remain regression evidence for the existing behavior; they cannot qualify the
new model as hardware execution. Record actual results and limitations in the
[Executable Replacement Model Log](../../notebook/Development/2026-10-19%20Executable%20Replacement%20Model%20Log.md).

## Live integration remaining

Authenticate the actual suspended caller and fresh sealed invocation, immutable
source and mapping owners. Implement one shared expected-permission resolver
for complete tree verification, user copies, root selection, reconfiguration
and terminal cleanup. Retain owners through every uncertain hardware store and
invalidation; do not reuse the terminal graft-withdrawal state as a temporary
detach. A fresh one-use completion must precede any caller resume.

Before accepting live executable replacement, review the emitted kernel/native
code and stack bounds, then qualify real instruction fetch from the RX region,
write-protection and NX-neighbour faults, and cleanup across failures. Another
interpreter syscall alone cannot establish execution from the new region. A
separate controlled source-backed fixture may exercise the same adapter without
altering the Linux interpreter or its libc. User W^X does not establish global
physical W^X while a supervisor RW/NX alias exists.

Dynamic linking, main executable entry, Linux exit and Electron/Antimatter
execution remain beyond this checkpoint.
