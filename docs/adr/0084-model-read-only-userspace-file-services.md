# ADR 0084: Model read-only userspace file services

- Status: Accepted portable host foundation; native integration pending
- Date: 2026-10-11
- Builds on: ADRs 0061, 0062, 0073 and 0083

## Context

The unchanged interpreter's fourth syscall requests a live hwcaps variant of
`libc.so.6`. ADR 0083 qualifies authenticated pathname capture, but supplies no
namespace answer, descriptor or file bytes. The existing Linux record-device
policy and native document handles do not implement regular-file offsets or
publication transactions.

Adding one literal absence per request would not establish the reusable file
service needed by the existing Linux Antimatter/Electron application. The next
foundation is a userspace model with explicit namespace coverage, immutable
files, task-bound descriptors and failure-atomic publication. Its first consumer
is a host proof using the complete pinned libc file. Native integration remains
separate: the current guest source profile holds at most 512 KiB and admits only
the existing executable/interpreter/startup source; this libc is 2,190,608 bytes.

## Decision

Add an opt-in, allocation-free, safe Rust module in the userspace veil protocol
crate. Keep it absent from default features and the running native/Linux veil
profiles. This selects an internal portable service contract, not a new native
operation, wire format or general filesystem.

Trusted configuration supplies an explicit complete namespace scope and exact
immutable file bindings. Exact canonical names inside that scope may resolve to
a held file or declared absence; paths outside its coverage or using unsupported
syntax remain unsupported. Absence follows the configured guest view, never
the host's filesystem or an inferred loader search order. Linux byte strings
remain distinct from the native UTF-8 naming direction. No fixed path length,
component length or depth array becomes part of this model's semantics.

Borrow immutable file bytes with their object/version identity, retaining their
lifetime through every open descriptor. Identities and digests are references
and integrity evidence, not authority. An opened descriptor continues to refer
to its original bytes if another namespace view replaces that name.

Bind each service to a separately authenticated task incarnation and a fresh
service incarnation. These facts cannot come from the Linux invocation's raw
arguments; its current copied record contains no authenticated task identity.
The trusted controller must supply fresh incarnations after recreation. Choose
descriptor capacity and the first available integer explicitly; do not infer
that standard descriptors exist. Internal generations never wrap and protect
stale internal handles. An ordinary reused Linux integer alone cannot express
an old handle's generation or reject a caller's stale intent.

Prepare open, sequential read and close effects before publication. A synchronous
trusted publication callback receives the complete result and any immutable
read bytes. Commit local descriptor/offset/request state only after it reports
committed. Known rejection changes no committed state and permits retry.
Unknown publication fences the service and revokes its local descriptors;
it must not be represented as a retryable absence of effects. Reads advance by
the actual delivered length, with explicit zero-length, EOF and short-read
behavior and per-invocation byte budgets. Caller retirement revokes authority.
Callback unwinding also fences the instance. The last committed sequence is a
watermark, not a request ledger: rejected attempts and local validation errors
do not bind a request body. The adapter must authenticate the exact pending
retry and account separately for completed error replies.

The callback is an integration obligation: the model cannot make arbitrary
external memory writes and a guest reply atomic. A future native adapter must
authenticate the current suspended caller, preflight destination memory, stage
bytes and revalidate the exact reply before publication. The present completion
gate resumes the veil only at the next caller invocation. That adapter must
reconcile the pending local effect before handling that invocation and contain
the dependent caller if the veil faults or loses acknowledgement. Transparent
recovery or replay after an unknown outcome is not provided here.

## Qualification and limits

Require hostile namespace, ownership, generation, sequence, capacity and
publication tests. Cover long and non-UTF8 Linux names, exact coverage boundaries,
independent descriptor offsets, close/reopen, stale internal handles, changed
namespace views, representation exhaustion, cancellation and unknown outcomes.

A host command reads a bounded regular input once, checks its configured SHA-256
and exercises the same service against those held bytes. Bind the complete
libc snapshot to its base pathname in an explicitly configured library scope;
the observed hwcaps candidate is absent in that test view. Reconstruct every
byte through successful bounded reads by two independent task incarnations and
retain delivered bytes and transaction evidence for an independent comparison.
Test failed publication, EOF, short/zero reads and lifecycle rejection. Execute
neither the input ELF nor a host application. Host file-size, descriptor and
transfer budgets are proof limits, not global native naming or storage limits.

Run required and affected regressions under the existing serialized memory
guard. Preserve and requalify the existing live-path guest proof separately;
this model does not advance its three returns or provision libc into the guest.

Native integration needs a separately pinned file source with a reviewed local
storage budget, authenticated bounded source reads, and staged caller copyout
joined to reply completion. Holding libc does not require mapping the entire
file into the Linux caller. This first model borrows a complete immutable byte
slice. Native reuse requires a separately admitted immutable mapping or a
chunk-readable source backend that preserves publication and offset semantics;
the native 8 KiB data profile cannot hold the full file. This is a separate
integration step. The existing bootstrap stack reserve also limits by-value
transcript growth; expanded guest observations need a fresh emitted
stack review. General traversal, symlinks, writes, stat/mmap, executable mappings,
dependency startup, Linux exit and Antimatter launch remain unqualified.

## Qualified checkpoint

The pinned 2,190,608-byte libc was reconstructed exactly through each of two
independent descriptor tables. The independent host verifier checked all 2,739
read publications and both retained streams. Sixteen protocol tests, 359 xtask
tests, strict host/freestanding lint and all 29 required/affected regression
commands passed on the final frozen source.

Fresh TCG/KVM runs at 96/320 MiB retained the existing three-reply loader proof.
All five baseline images remained byte-identical. A host pressure spike stopped
an earlier regression attempt; the unchanged guard was retained and a fresh
complete run passed. No OOM kill was observed. VirtualBox still refused the
existing CPU capability gate and powered off. The separate host and regression
receipts, preserved failures and exact audit bindings are recorded in the
[Read-only File Service Log](../../notebook/Development/2026-10-11%20Read-only%20File%20Service%20Log.md).

Acceptance covers this portable model and host proof. It does not qualify a
fourth guest reply, a guest file open/read, libc startup or Antimatter execution.
