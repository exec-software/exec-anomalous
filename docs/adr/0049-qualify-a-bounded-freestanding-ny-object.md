# ADR 0049: Qualify a bounded freestanding Ny object

- Status: Proposed
- Date: 2026-10-03
- Scope: One pinned scalar userspace fixture and its inspected startup shim.

## Context

[ADR 0048](0048-prove-bounded-application-cpu-copy-and-fault-profile.md) supplies
retained user mappings, an eager x87/SSE2 CPU owner, task-bound copies, and
selected task-local faults. The next gate toward native Antimatter is actual Ny
code under that profile. The installed Ny SDK emits Linux host-targeted ELF
objects and a generated C `main(int, char **)` wrapper. Its ordinary runtime
uses hosted allocation, operating-system services, and TLS. Linking that
runtime would silently exceed the qualified kernel profile.

## Decision

Add the opt-in `ny_object_test` fixture, consuming the shared Anomalous Ny
runtime generation
`c541621251d8502d7d7c547d24f10322bc894e5943dc21a1abaaed59c71fd385` from Ny
revision `2a45b0bb3891db6e32db5bc9d8786223c95c9687`. The generation is produced
by `scripts/refresh-ny-runtime.sh` and consumed from the shared `runtime/`
directory. As of this revision it is an explicitly marked development SDK: the
Ny worktree carries in-progress Matrix front-end work, so the manifest records
`worktree_dirty: true`. Pin the manifest and consumed executable contents before
compiling, so the dirty marker does not weaken the object proof. Compile checked
source without standard-library lookup or bundling, then assemble its NIL in
unmanaged mode. Retain the original emitted object without rewriting it. Run the
same source through the pinned interpreter as an independent route.

The fixture initializes Ny globals, performs 50,000,000 integer increments and
exact binary floating-point additions, checks both results, and returns
50,000,000. A separate narrow assembly shim calls the generated C wrapper with
zero arguments and implements only its observed ABI-version, context-begin,
context-end and failure hooks. Each supported runtime hook checks its permitted
call order and stack alignment. Startup establishes a 16-byte-aligned stack
before calling the generated C wrapper; each callee observes the System V
8-byte entry offset. Ny has no red-zone-disabling CLI option. Its userspace red
zone remains intact because interrupt entry uses the separately owned TSS
privilege stack. The compiler's unconditional unused string helper imports
`strlen`, `calloc`, and `memcpy`; distinct fatal trap stubs satisfy these
link-time references without supplying a hosted compatibility library. Reaching
any of them fails the proof.

The host inspects the raw relocatable object, complete linked executable,
opaque embedding wrapper and final kernel payload. Admission requires the
observed x86-64 instruction profile, bounded sections, known imports and
link-time relocations. The final executable has no unresolved imports, TLS,
relocations, dynamic linker, GOT/PLT, constructors, allocator or unwinder.
Only inspected unwind metadata is discarded. This fixed-fixture inspection is
not an arbitrary hostile executable validator or a compiler target guarantee.

The linked user image has one file-backed RX page at `0x400000`, a private
128-byte initialized header at `0x800000`, bounded zero-filled globals beginning
at `0x800100`, and the existing two-page data and guarded stack regions. The
nonzero header initializer must survive extraction, immutable embedding, owned
population and user readback. Source initialization must establish the Ny
globals after the startup verifies the initial zero-fill extent. No writable
code or serialized kernel loader is introduced; parsing and linking stay in
the host tool.

Two scenarios each retain two independent task/root/stack owners. In the first,
task 0 receives an incompatible runtime ABI version and follows Ny's generated
failure branch. In the second, task 0 completes the actual computation and its
context-end shim deliberately fails. In each scenario task 1 then runs the same
Ny program successfully. Failure messages and results cross the existing
current-task checked-copy boundary. The selected fault is an exact armed user
UD2 after the validated failure gate, with IF clear only across that terminal
window; unexpected faults remain fatal to this proof.

Timer entries capture and restore the current task's x87/SSE2 state in place.
The proof requires actual PIT observations within the generated computation,
not only within assembly startup or a waiting loop. This slice adds no scheduler
preemption claim; ADR 0048 retains that separate evidence. The ordinary sole-
current completion and fault-to-peer transitions need no new scheduler API.

All owners remain live until terminal tasks have returned to the retained
bootstrap continuation. Restore bootstrap CR3, TSS and original kernel CPU
state, clear CPU images, withdraw contexts and reclaim mapping/stack leases
before publishing successful cleanup. Bind release accounting to boot evidence.
Private pointers, source paths, payload bytes and failure messages do not enter
serial records; only bounded evidence and counters are public.

## Validation and limits

Qualify object/link/wrapper rejection tests and a strict native transcript
oracle. Require TCG Intel-baseline and actual-host KVM runs at 96 MiB and
320 MiB, exact resource restoration and normal-image exclusion, plus the full
repository regression matrix. Retain failed runs as evidence. Builds use two
jobs, a 6 GiB scope cap and a host/scope OOM watcher.

Keep the default `make check` independent of runtime installation: run pure
host artifact/transcript tests and the Ny kernel feature checks there. The
explicit `ny-object-inspect`, `ny-object-image` and `ny-object-smoke` targets
require the pinned shared runtime and fail when it is unavailable or changed;
they never silently skip qualification. The full Ny-slice matrix includes both
TCG and KVM native targets, each of which rebuilds and inspects the actual
object/image pipeline.

Qualification on 2026-10-03 passed all 30 targets on 272 unchanged frozen source
files, including both RAM sizes under TCG's Intel baseline and actual AMD-host
KVM. Each native run accepted 13 records and restored 24,149/81,493 free frames;
the two backends used identical images. The final full check passed 440 default,
449 Ny-feature and 483 all-feature kernel tests, plus 119 xtask tests. No OOM
event occurred under the 6 GiB cap. Normal/workspace images were restored and
remain unchanged. No VirtualBox run was attempted in this slice; Threshold is
unchanged. That evidence qualifies the bounded fixture for SDK generation
`456a9a11…` while the ADR remains Proposed.

Re-qualification on 2026-10-14 followed the move to the shared `runtime/`
generation `c5416212…` (a development SDK built from a dirty Ny worktree that
carries the Matrix front end). `ny-object-inspect` and `ny-object-image`
reproduced the pinned object, link, wrapper and image inspection profile, and
`ny-object-smoke` passed the TCG Intel-baseline boot at 96 MiB and 320 MiB
(restoring 24,149/81,493 free frames, identical to the prior run, with both
scenarios, failure containment and cleanup accepted). The actual-host KVM run
and the wider regression matrix were not repeated in that session, so this is a
development re-pin rather than a fresh release qualification.

This establishes one Ny source/object/startup composition. It does not qualify
the hosted Ny runtime, a general Anomalous compiler target or SDK release,
allocation, strings, TLS, AVX/XSAVE, unwind recovery, an application service ABI,
Antimatter, HAG or privileged Ny migration. Existing Rust remains authoritative.
The next slice is task-bound surface/input services and a native Antimatter
frontend using a bounded software rendering path; workspace persistence follows.
Further Ny language/runtime requirements must be qualified as the application
actually needs them.

The [Ny Object Log](../../notebook/Development/2026-10-03%20Ny%20Object%20Log.md)
records implementation evidence and the exact qualification limits.
