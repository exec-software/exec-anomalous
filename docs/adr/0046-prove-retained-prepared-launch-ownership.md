# ADR 0046: Prove retained prepared-launch ownership

- Status: Proposed
- Date: 2026-10-03

## Context

[ADR 0044](0044-prove-owned-low-address-user-execution.md) proves two actual
GPR-only ring-3 tasks over retained low-address roots. Its harness manually
assembles mappings, initialized payloads, circles, and tasks, then tears them
down after successful execution. It does not retain that whole lifetime across
partial launch failure or cancellation of an unstarted task.

[ADR 0045](0045-select-native-antimatter-ny-and-hag-direction.md) selects native
Antimatter as the next major userspace milestone. An application will exercise
the ABI through real work, including failed launches and restarts. This slice
establishes the ownership prerequisite before introducing new ABI fields,
artifact parsers, or editor services.

## Proposed decision

### Prepare without publishing a runnable task

Add an internal retained `prepared_launch` owner around the existing mapping
stage, admitted entry/stack geometry, circle identity, and attachment progress.
Limit this proof to the existing three-region GPR-only profile: one RX code
page, two RW+NX data pages, and two guarded RW+NX stack pages with explicit
kernel residency. Plans remain inert and format-neutral.

Preparation rechecks admission against the actual mapping plan, builds the
inactive mappings, copies and verifies the declared payload through a trusted
bounded callback, revalidates the mapping tree, and attaches a new circle's
address-space/mapping ownership. It publishes no task. A failed preparation
retains all acquired resources and requires cancellation rather than resuming
with a different payload.

Task creation currently makes a task ready and dispatchable. Therefore explicit
commit performs that publication only after successful preparation and exact
attachment checks. Its final operation creates and retains the task identity;
no fallible operation follows publication. Rejected commit keeps the prepared
owner available for retry or cancellation. Principal and origin remain synthetic
fixture metadata supplied separately from the load plan, not authenticated
identity or authority derived from a file.

### Cancel before reclaiming memory

Add the narrow task transition `cancel_unstarted`: only `ready` without an
attached execution context may become `exited`. It keeps identity/binding and
circle accounting until ordinary destruction. Invalid transitions and identity
mismatches fail without mutation. This extends the earlier task lifecycle for
an execution attempt that never ran; cancellation does not simulate start/exit.

The retained launch owner cancels partial preparation, a committed unstarted
task, or an already terminal task. It rejects running or scheduled consumers
and unexpected circle attachments before reclaiming mappings. Root registration,
hardware translations, and saved continuations must already be withdrawn by
the separately retained activation session.

Record each successful cleanup step before attempting the next: verified mapping
release, mapping/address-space child detachment, task destruction, then circle
destruction. Mapping clear/release failure preserves every unreleased lease and
supports retry; a cancelled task stays non-runnable during that retry. Existing
task/circle record-clearing failures deliberately leave tombstones and remain
terminal: poison and retain provenance rather than retry a stale identity.
There is no fallible automatic `Drop` cleanup.

### Use the owner in the native proof

Before the existing two-task user execution, inject allocation clearing, table
population, payload initialization, and circle-exhaustion failures. Prove
prepared cancellation and each cleanup-clear failure after committing a ready
task. Require no task publication before commit, cancellation before failed
cleanup, exact accounting restoration, stale-generation rejection, and an
unchanged bootstrap root.

Then prepare and commit the two original users through the same owner. Retain
its mapping borrows through root activation, four resumed reports, B's contained
supervisor-read fault, A's continued execution/exit, and bootstrap restoration.
Use the same cancellation/terminal cleanup path afterward. A numeric success
counter without actual user execution is insufficient.

## Validation contract

- Host tests cover task transition atomicity, launch admission/publication,
  injected preparation/cleanup failures, retry ownership, and rejection of
  unexpected live attachments.
- Native memory smoke runs at 96 MiB and 320 MiB under TCG and KVM. Preserve all
  preceding records and add `kernel.memory_test.launch.verified` after
  `user.released`, before the final `passed` event.
- The new record requires exactly four allocation failures, one table-write
  failure, one payload failure, one circle failure, one prepared cancellation,
  five unstarted cancellations with five cleanup failures/retries, and two
  successful execution commits. Boolean fields attest no preparation task,
  cancellation before cleanup, commit before entry, and exact retry restoration.
- The strict host verifier rejects missing, duplicate, reordered, unknown,
  mistyped, changed, or extra evidence, while QEMU exit status independently
  confirms completion. Mutation tests include every new field and event.
- Run the complete regression matrix and normal-image exclusion checks. Record
  actual image hashes, source freeze, host memory peak, and OOM counters.

## Recorded evidence

All 24 regression targets passed, including the complete host check and final
memory/dispatcher/workspace KVM repeats, in 1,059.31 seconds of combined target
time. All 254 frozen non-documentation source/configuration files remained
unchanged. Focused tests passed four cancellation, eight owner, and seven
memory-transcript tests plus freestanding Clippy; full suites passed 410 default
kernel, 453 all-feature kernel, and 92 xtask tests.

TCG and KVM memory runs at 96 MiB and 320 MiB each emitted 24 structured records,
including 21 selected lifecycle records. Both backends restored exactly
24,148/81,492 free frames and produced identical kernel-memory evidence. Their
16 MiB memory-proof images share SHA-256
`943f4ba8162db082c50416f14bd991e2bbbc617f13b29ae9deda2bd0ee4d5ed2`.
Normal and workspace images remain byte-identical to the recovery checkpoint;
direct normal-image exclusion also passed.

Peak scope memory was 4.00 GiB with a 6 GiB hard limit and two build jobs; the
host retained at least 16.41 GiB available. The 4 GiB soft limit caused reclaim,
with no hard-limit or OOM event and no increase from the host OOM counter of zero.
The [Prepared Launch Ownership Log](../../notebook/Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
retains the evidence locations, resource measurements, and next integration
gates. This ADR remains Proposed; measured fixture behavior does not establish
the general application ABI.

## Consequences and remaining limits

This closes a bounded launch-lifetime composition. It does not establish a
general application ABI, scheduler integration for these owned roots, arbitrary
user-copy, general exception recovery, TLS/FPU ownership, permanent kernel
placement, global W^X, a native Ny runtime, or Antimatter execution. The payload
callback is a trusted kernel fixture, not an untrusted executable loader.

Antimatter remains the first intended application test of the ABI. The next
foundation work must combine this retained owner with sustained dispatch and
the supported application CPU/copy/fault profile before defining its surface,
input, and workspace-service contracts.

## Alternatives

- Publishing a ready task during preparation exposes a runnable partial launch.
- Adding a new global prepared-task state would expand every lifecycle consumer
  when delaying task creation suffices for this bounded profile.
- Starting and immediately exiting a task to cancel it falsely records execution.
- Best-effort destructor cleanup cannot retain ownership after fallible clearing.
- Serializing a launch ABI or adding ELF parsing now would freeze representation
  before the ownership failure paths have native evidence.
