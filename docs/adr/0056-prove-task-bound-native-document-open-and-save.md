# ADR 0056: Prove task-bound native document open and save

- Status: Proposed; bounded native proof qualified
- Date: 2026-10-04
- Related: ADR 0045, ADR 0050, ADR 0051, ADR 0055

## Context

The qualified native startup proof supplies document bytes and private working
memory, but it has no document service. Antimatter needs an explicit saved
baseline and an acknowledgement of the exact submitted snapshot. A transport
acknowledgement only releases copied packet storage. It cannot establish that a
document service accepted bytes, that the editor's current buffer was saved, or
that storage is durable.

The next native contract must distinguish a save request from a newer edit,
reject closed or obsolete handles, and preserve uncertainty when a service
fails before its semantic reply arrives. These semantics precede a comparison
of the same contract through the separately qualified ELF/native personality.

## Decision

Add an opt-in native document proof with two separately owned ring-3 tasks: an
editor and a service combining the bounded RAM document store with the existing
software surface consumer. Four independently bound copied-packet routes carry
surface, input, document requests and document replies. The kernel authenticates
current task generations, owns route and memory authority, copies checked bytes,
and contains faults. It does not parse document records, choose documents,
implement editor state or decide save policy.

The new `anomalous-document-protocol` crate is safe `no_std` userspace code. It
defines one document of at most 31 printable ASCII bytes, one open handle, and
canonical 128-byte requests and replies for open, read, save and close. Every
request carries an opaque document identity and a correlation request number.
An open reply adds a handle generation, current document version and exact bytes.
Save carries that handle, the expected version, editor revision and exact
snapshot. A successful reply echoes all those fields and the exact accepted
bytes, with a new version equal to the expected version plus one.

The store receives the authenticated client task identity separately from the
record. Record fields never authenticate their sender. Each proof scenario has
a distinct controller-selected document identity; it denotes that RAM service
incarnation, not a filename or global namespace. Within one service, every
successful open mints a new nonwrapping handle generation. Close revokes the
current handle without deleting committed bytes. Reopen returns those bytes and
a fresh handle. A replacement service starts a new store and document identity;
this slice defines no global identity allocator or process-relaunch persistence.

Initial document version is one. Saves check the exact owner, document, live
handle, strictly increasing save request number and optimistic expected version
before any mutation. The save request sequence is independent of open/read/close
correlation numbers and restarts only with a new handle. Successful same-byte
saves still advance the version. All errors preserve store bytes, version,
handles and counters. Exhausted generation/version/request spaces never wrap.
Duplicate saves are rejected, not treated as an implicit successful retry.

A separate opened editor state leaves the preceding editor layouts unchanged.
It retains one immutable pending snapshot and an exact saved-byte baseline.
Only a reply matching the pending document, generation, request, base version,
editor revision, next version and bytes updates that baseline. The current
buffer is independent: an edit or undo after submission remains dirty if it
differs from the acknowledged snapshot. Dirty state compares bytes with the
last acknowledged baseline, not just revision numbers. An unknown save outcome
additionally forces unsynchronized/dirty presentation, including when an undo
returned the current buffer to the previously acknowledged bytes: the service
may already hold the pending snapshot instead. A matching definitive rejection
can clear a pending request without marking anything saved. A service failure
preserves the pending snapshot as an unknown outcome. A later exact success may
resolve that uncertainty; a rejection cannot undo a possibly committed save.
A duplicate-save `stale_request` rejection is not a definitive negative result
for the original attempt and must not be used to resolve its unknown outcome.

The service commits a save before sending its reply. Failure between those
steps can leave a committed RAM snapshot without a client acknowledgement.
There is no cross-task transaction, durable write, recovery journal or
idempotent retry cache. Normal read and close/reopen demonstrate the stored
bytes while the same service survives. Fault cases separately demonstrate
uncertainty before commit and after commit but before reply. A whole-pair fresh
launch resets RAM state; it is not described as recovery of saved content.

## Experimental formats and resource bounds

Document records use magic `ANDC`, version one, exact length 128 and little-endian
integers. Byte 8 identifies open/opened/read/read-result/save/saved/close/closed/
rejected; byte 9 is zero except for a structured rejection. Bytes 16, 24, 32, 40,
48 and 56 begin document, handle generation, request, base version, new/current
version and editor revision respectively. Byte 64 is the text length, followed
by at most 31 bytes. All reserved bytes and unused text tail bytes must be zero.
Operation-specific field validation rejects inconsistent requests and replies
before the store sees a record. Rejections echo the original request tuple and
bytes without disclosing another document's content.

A separate `ANDS` startup descriptor preserves the preceding `ANRT` format.
It has version one and length 128. Role, scenario, arena base/length, four
task-local route handles, document identity and the configured client task/
generation occupy successive eight-byte fields from byte 8 through byte 88.
Byte 96 holds seed length and bytes 97 through 127 hold seed text with a zero
tail. Only the service receives seed text; the editor obtains content by open.
The copied descriptor describes the launch, while retained kernel task/route
tables remain authoritative. All four handles are nonzero and distinct.

Reuse the explicit four-region native arena ownership profile: eight RX code
pages, two private RW data pages, eight private RW/NX arena pages and two stack
pages per task, with existing guards. The pair still owns 66 frames in twelve
leases. Four bounded channels and both task lifetimes are retained until orderly
close or fault containment. No live mapping growth, generic filesystem syscall,
new foreign syscall policy, HAG path or increased staging bound is introduced.

## Acceptance

Host tests must cover canonical records and startup data, owner and task-
generation mismatch, closed/stale/replaced handles, version conflict, replay,
counter exhaustion, exact accepted bytes, rejection atomicity, late snapshot
acknowledgements after newer edits, and unknown-outcome behavior. Malformed
records must not enter or mutate the store. Existing native editor layouts,
startup records and three-region launch behavior retain their prior tests.

The native proof must open its bytes through the independent service, edit,
submit a save, alter current state before receiving the exact snapshot reply,
verify correct dirty state, read the committed bytes, close and reopen with a
fresh handle, and compare semantic surfaces and the final frame. Live probes
must reject wrong routes, stale/replaced document identities, closed/stale
handles and conflicting versions without unintended store effects.

Before-commit and after-commit service fault markers must be distinct inspected
instruction sites, reached with a delivered save request and no queued reply.
The surviving editor must observe closed routes and retain an unknown pending
snapshot without becoming clean. Both cases must return all task, circle,
channel, mapping, CPU and allocator ownership. A later fresh launch must check
zeroed private arenas and independently open its new service's document.

Qualify both 96 MiB and 320 MiB guests under TCG/Intel and KVM/AMD, including
independent captures, exact serial records, emitted artifact/stack inspection,
prior required regressions and restored development images. Compilers and QEMU
remain serialized with capped build parallelism and recorded memory/OOM checks.
Record actual counts and hashes only after qualification completes.

## Qualification

The bounded native proof passed qualification, including host reference and
model/protocol checks, native execution and prior regression coverage. The
[Native Document Log](../../notebook/Development/2026-10-04%20Native%20Document%20Log.md)
retains the measured receipt and scope limits. This result qualifies the fixed
RAM service experiment; it does not stabilize this proposed ABI or prove
saved data across owner destruction.

## Limits and follow-up

This is a bounded Rust document contract and RAM service, not a filesystem,
durable save, full Antimatter/Electron port or completed Ny runtime. Document
identities and scenario values are proof scoped. It does not establish a stable
external ABI or support concurrent editors, multiple documents, service
discovery, arbitrary paths, Unicode, retry recovery or saved bytes surviving
service/process destruction. Ny remains the official language direction.

The next comparison exercises the same native document contract through the
qualified ELF/native personality. A surviving document service across editor
relaunch, followed by a bounded filesystem-backed implementation with explicit
durability semantics, remains necessary for the broader open/edit/save/relaunch
product gate in ADR 0045. Windows and macOS personality work remains separate.
