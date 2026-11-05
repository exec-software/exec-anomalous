# ADR 0087: Return native file-service replies to the Linux loader

- Status: Accepted for the bounded native file-reply integration
- Date: 2026-10-13
- Builds on: ADRs 0079, 0084, 0085 and 0086

## Context

The unchanged Linux interpreter can enter the native veil and receive three
startup replies. A separate immutable libc source can reach native userspace
in bounded chunks. The portable read-only file service already owns descriptor
generations, independent offsets and publication ordering. The next step joins
these mechanisms so actual Linux file requests can receive bytes from that
source without moving filesystem or Linux policy into the kernel.

## Decision

Extend the removable loader-entry proof profile. The native veil configures a
complete `/lib/x86_64-linux-gnu` subtree containing the retained `libc.so.6`
snapshot. It derives absence from that namespace, including narrowly admitted
queries for absent candidate directories. The Linux adapter accepts one terminal
slash for metadata lookup while retaining the exact captured pathname. Existing
files, implicit directories, repeated slashes, dot components and unsupported
metadata queries remain unsupported. The interpreter determines request order; neither
the kernel nor the proof controller prescribes a sequence of Linux errors.

The service uses the bounded immutable-source backend through native operation
23. Its existing 4 KiB buffer is reused for publication. The full-source stream
proof remains a separately accounted phase. The native writable mapping stays
8 KiB; neither the whole libc nor an unchecked host pointer becomes a user
mapping in the Linux caller.

Operation 24 copies an 80-byte `ANFCTX01` context bound to the current sealed
request. It carries the pending sequence, caller task and circle generations,
service task generation, fresh service incarnation and last accepted sequence.
These numbers identify an authenticated deployment; they are not capabilities.
The kernel still checks the current task, selected root, owned frame, scheduler
authorization and exact suspended caller before any privileged operation.

Operation 19 stages a bounded copyout from the native veil after preflighting
the complete caller destination. Operation 20 validates the exact retained
sequence and token, revalidates the destination, copies the retained bytes and
commits the prepared scheduler reply. All recoverable validation precedes mutation
and scheduler commit. Actual destination readback uses the same admitted
physical spans without reopening a fallible authorization step after mutation.
A rejected request leaves the caller blocked and preserves its frame and bytes.

The portable publication callback remains suspended inside operation 20 while
the Linux caller runs. When the next invocation wakes the veil, its delayed
acknowledgement must match the prior sequence and staged length. Only then may
the portable service commit its descriptor offset and sequence watermark,
before handling the new request. Unknown acknowledgement fences the service;
an accepted old reply must never be replayed to resolve an uncertain local
offset.

Within an established pending-call or waiting-service relationship, fault
containment retires both a failed service and its dependent caller while
allowing an independent peer to finish. This includes faults after staging but
before publication, faults after an accepted reply but before local state
reconciliation, and caller faults while the service waits for its next wake.
Retirement clears retained stage/request/scratch state and native private
buffer storage before releasing the owned mappings and source leases.
Faults before the initial handoff and general task-fault recovery remain outside
this qualification.

Native scratch is explicitly wiped with volatile stores after normal service
returns. A suspended callback may still borrow its publication bytes, so no
aliasing-unsafe wipe is performed through that shared reference. Fault cleanup
owns the physical storage and clears it without relying on Rust destructors.

## Qualification boundary

The same image runs three fresh owned lifetimes: normal delivery followed by
unsupported retirement; a fault after staging a read; and an unknown local
publication result after an accepted read. Each lifetime has a fresh service
incarnation, complete teardown and a surviving independent peer. Expanded
observations live in retained kernel BSS, not by-value bootstrap stack frames.

A strict independent host oracle checks the actual ordered syscall transcript,
path bytes, descriptor behavior, complete saved frames, immutable source bytes,
staged copyout and actual caller readback. It must not count a run that stops
before a successful open and read as completion of that milestone. Fresh
emitted stack/control review precedes guest execution. Memory and path budgets
belong to this proof operation and introduce no global path ceiling.

This increment does not map libc into the Linux process, complete dynamic
linking, finish `true`, or launch Antimatter. Those claims require subsequent
observed requests and independently qualified services.

## Accepted evidence

TCG and KVM at 96/320 MiB run identical retained image bytes and qualify all
three fresh lifetimes. The normal unchanged interpreter opens libc as fd 3,
reads its first 832 bytes and reaches the actual next unsupported syscall,
`pread64(fd=3, count=840, offset=64)`. Fault cases distinguish untouched caller
bytes before publication from accepted copyout followed by local service fencing.
Six malformed-source provisioning cases and all 29 required/affected checks
pass. The CPU workspace image is restored; the baseline review classifies the
editor's diagnostic line-number changes without admitting code or layout changes.
No OOM kill is observed. VirtualBox's CPU capability refusal and poweroff remain
separate from successful QEMU execution. The
[Native File Replies Log](../../notebook/Development/2026-10-13%20Native%20File%20Replies%20Log.md)
retains exact source/artifact identities, earlier failures, emitted reviews,
physical transcripts and final audit receipts.
