# ADR 0091: Release allocator borrows before userspace

- Status: Accepted; bounded QEMU qualification completed 2026-10-15
- Date: 2026-10-13
- Builds on: ADRs 0079, 0089 and 0090

## Context

The retained runtime mapping owner needs ordinary allocator access from a
suspended-caller service path. Before this change, the loader proof entered
userspace from inside its bootstrap allocator callback. The allocator rejects a
second borrow, so retaining that callback across guest execution prevents a
service from acquiring memory. Saving an allocator pointer would bypass its
exclusive-borrow contract.

## Decision

Separate loader preparation, guest execution and teardown into explicit phases.
Preparation initializes the retained BSS owners and activates their leases under
one lexical allocator borrow. That callback returns before initial task
selection, dispatch publication or guest entry. Only copied accounting survives;
no allocator reference or pointer crosses the transition.

Guest execution uses the existing stable BSS address and short runtime borrows.
No mutable runtime reference spans the nonlocal guest transfer. Terminal dispatch
restores the bootstrap root, privilege stack and CPU state and withdraws active
dispatch before returning to the bootstrap continuation. Validate that continuation
before taking a fresh allocator borrow for withdrawal and cleanup. Sources remain
owned until all dependent mappings have been released.

Keep `memory::with_frame_allocator` and its exclusive state transition unchanged.
Its callbacks remain synchronous: a callback cannot dispatch, enter userspace or
perform a nonlocal return. Allocator access supplies mechanism, not authority to
map memory into an arbitrary task.

## Qualification boundary

Once per proof lifetime, after authenticating the existing native context request
and preflighting its destination, acquire the allocator through its ordinary
entry point. Compare its accounting with the prepared live snapshot. Attempt a
nested ordinary borrow and require `allocator_busy` without executing the nested
callback. Return from the first borrow, then acquire again to prove release.
Preserve the sealed pending sequence, request and complete saved caller frame.
This probe reads allocator metadata; it does not allocate or publish a mapping.

Kernel observations record two successful service borrows and one nested denial
in every lifetime. Host transcript tests reject missing, mistyped or incorrect
observations, while fresh TCG/KVM execution supplies the actual service evidence.
The host allocator entry is a stub and cannot establish physical reacquisition.
Fresh emitted stack/control and instruction review precedes guest execution.

Preserve all seven existing file-service lifetimes and their exact effects:
the normal interpreter still makes ten Linux returns and stops at its first
libc `mmap`; the six injected fault windows still retire their dependent pair,
preserve the peer and restore owned resources. No native wire operation, Linux
return, mapping publication or Antimatter/HAG execution is introduced here.
All required and affected regressions remain serial under the unchanged OOM
guard. Live mapping preparation/publication and the combined root, copy and
circle ownership contract remain the next integration work.

## Qualification on 2026-10-15

The implementation retained in `86af265` is qualified on isolated
checkpoint `6e30506` (code baseline `14e9083`). A fresh 558-file source freeze,
378 xtask tests, 626 focused kernel tests, strict lint, image capture and all
thirty required/affected regression jobs pass. Fresh kernel emitted control and
instruction reviews precede four boots of identical reviewed artifacts under
TCG and KVM at 96/320 MiB. All seven lifetimes per boot record two ordinary
service borrows and one nested denial, with preserved caller state and restored
resources. The strict transcript oracle and independent physical crosscheck
both pass. Six malformed source-provisioning cases are refused before Linux
execution; those refusal logs do not independently measure final cleanup.

The native fixture has identical loaded bytes and fifteen identical deployment
files after explicitly retaining its startup seed. The isolated rebuild differs
in nonallocated ELF metadata; a separately reviewed exact-metadata binding
establishes loaded-image equivalence without claiming whole-ELF equality or
reusing prior kernel or guest qualification. Emitted stack bounds leave 28,720
bytes on the bootstrap stack and 10,592 bytes on the busiest privileged path.

The final audit covers 80 attempted jobs in 17 serial scopes and preserves all
failed attempts. No new OOM kill or pressure abort was observed in these scopes;
the regression build recorded 4,245 soft-limit events without reaching the 6 GiB
hard limit. All five development baseline images remain byte-identical.
VirtualBox separately refuses `application_cpu_amd_error_pointers` and is verified
powered off; its loader path is not qualified. The normal Linux boundary remains
ten returns followed by unanswered sequence 11, the first libc `mmap`.

Claude's later ADR 0094 shared-core work is outside this pinned qualification.
The next bounded step is authenticated retained mapping preparation and
cancellation while the caller remains suspended, before mapping publication or
a Linux reply.
The [Allocator Service Lifetime Log](../../notebook/Development/2026-10-13%20Allocator%20Service%20Lifetime%20Log.md)
records exact receipts, attempts, coordination and evidence locations.
