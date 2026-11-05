# ADR 0047: Prove sustained dispatch over retained launches

- Status: Proposed
- Date: 2026-10-03

## Context

[ADR 0039](0039-prove-bounded-sustained-native-dispatch.md) proves bounded
timer-driven dispatch using the earlier private-root fixture.
[ADR 0046](0046-prove-retained-prepared-launch-ownership.md) separately retains
prepared low-address mappings, task/circle attachments, explicit task commit,
and cancellation. Neither checkpoint proves sustained scheduling while those
prepared-launch owners retain every live root, saved frame, and cleanup duty.

[ADR 0045](0045-select-native-antimatter-ny-and-hag-direction.md) selects native
Antimatter as the next major userspace milestone and the first intended actual
application test of the ABI. Compose the existing kernel ownership and dispatch
mechanisms before qualifying the CPU, copy, and fault profile that Ny startup
and native Antimatter will need.

## Proposed decision

### Retain the complete execution lifetime

Add a removable `owned_dispatcher_test` feature and a private `owned_execution`
aggregate. Retain two `prepared_launch` owners, two allocator-owned 16 KiB
supervisor privilege stacks, the checked physical high alias, and activation
registration/progress until explicit terminal cleanup. The same supervisor
alias reaches each privilege stack in either private root and the bootstrap
root. Numeric root and stack addresses are inspection metadata, not ownership
or independently transferable authority.

Keep preparation without a task and explicit ready-task commit. Source bytes
must be completely inside immutable linked rodata and the admitted initialized
code extent. Authority and trusted synthetic principal/origin metadata remain
separate from the inert, format-neutral mapping plan. Both launches use the
existing three-region GPR-only profile. Together with the privilege stacks,
the proof retains 36 frames: 18 data frames, 18 table frames, and ten active,
bound leases. It does not migrate legacy runtime owners or define a loader.

### Withdraw only an unstarted scheduler admission

Add `scheduler_table::withdraw_unstarted_context` for an exact ready task with
one queued initial context. Reject any global current task or outstanding
dispatch reservation, stale or wrong identities, saved contexts, other states,
and inconsistent queue/task/circle accounting before mutation. Precompute queue
removal, then perform the final fallible task/circle detachment before committing
the scheduler change. Preserve survivor order, the ring head, and dispatch and
preemption sequence cursors.

The corresponding task hook detaches only the ready unstarted context and its
circle child. It leaves the task ready; `prepared_launch` subsequently performs
`cancel_unstarted` before reclaiming mappings. This is a narrow rollback path,
not a general operation to unschedule running or suspended tasks.

Exercise partial pair admission with a real one-slot scheduler: admit the first
task, reject the second for capacity, withdraw the first, and cancel both
retained launches. Inject two privilege-stack clear failures during teardown;
each failed step keeps the remaining lease and physical alias reachable for
the next cleanup attempt. No fallible `Drop` cleanup is introduced.

### Dispatch, contain one exact fault, then reclaim

Run both users through 64 real PIT preemptions, 32 per task, with independently
advancing private counters and intact register/data/stack canaries. Nested user
calls leave both live stack pointers at their respective user stack top minus
24 bytes. Validate that noninitial return state against admitted initialized
code and guarded stack bounds on every relevant return path.

After those preemptions, permit only the exact armed task-0 `UD2` fault. Task 1
continues for 16 timer ticks with measurable progress and no artificial advance
of the dispatch sequence, then exits. The resulting totals are 80 timer ticks,
66 dispatches, 64 preemptions, one fault, and one exit. The faulted owner's
mapping and stack remain retained until both tasks are terminal; do not clear
the faulting stack while a handler or continuation can still depend on it.

Restore bootstrap CR3, TSS.RSP0, CR0.TS, and the separate kernel continuation
before withdrawing scheduler contexts, root registrations, and owned storage.
Revalidate mappings while they remain retained, record each successful cleanup
step, clear/release all leases, and remove the physical alias last. Clear/release
failures retain progress and support retry. A failed mapping revalidation is
retained and fail-closed; repeated `finish` is not a promised recovery path.
Existing terminal task/circle tombstone failures also remain terminal.

## Validation contract

- Host tests cover narrow task detachment and scheduler withdrawal, partial
  pair rollback, wrapped queue removal at head/middle/tail, survivor order and
  unchanged sequences, and atomic rejection of invalid or corrupt state.
- `make owned-dispatcher-smoke` builds a separate
  `build/owned-dispatcher-test.img` and runs at 96 MiB and 320 MiB. Repeat with
  KVM as part of final qualification, while retaining the earlier dispatcher
  and memory proofs as separate regression targets.
- Require eight boot records and exactly seven ordered
  `kernel.owned_dispatcher_test.*` records: `rollback`, `ready`, `progress`,
  `fault_contained`, `survivor`, `released`, and `passed`. Enforce exact keys,
  scalar types, counts, booleans, boot-bound restored free frames, and the
  57,344-frame difference between RAM sizes. QEMU completion status is checked
  independently from transcript contents.
- Mutation tests reject missing, duplicate, reordered, unknown, extra, and
  changed evidence. Normal-image exclusion rejects this feature's namespace,
  payload symbols, and canaries. Feature selection must preserve existing
  development-image precedence.
- Complete the full regression matrix with source/configuration freezes for
  each validation phase. If a narrow harness correction interrupts the run,
  identify every changed file, retain the original failure and diagnostic
  limits, and rerun the full host check plus affected native targets and all
  remaining targets. Earlier native results may count only when their kernel,
  target-specific harness paths, and dependencies are unchanged; record that
  comparison rather than claiming one unchanged freeze for the whole run.
  Record image hashes, memory limits and peak, OOM counters, backend coverage,
  and any unavailable platform evidence in the working log.

## Recorded evidence

All 26 regression targets qualified across documented phases in 1,491.42
seconds of summed selected target time: 14 unchanged initial native passes and
12 final-phase targets, including a fresh full host check. All 258 final frozen
non-documentation files remained unchanged. The only source change between
phases was display-capture readiness and two tests in `tools/xtask/src/main.rs`;
all kernel and other source/configuration files were unchanged.

The first phase stopped at the display-vessel transcript check after 15 passed
targets. Review found a pre-existing predicate that could match an incomplete
serial record, but the rejected transcript was deleted, so that failure's cause
is unconfirmed. Thirty direct boots of the unchanged kernel passed. The first
host check of the capture correction caught a JSON-fragment versus parsed-event
mismatch; the constant was corrected, both focused capture tests passed, and
the complete host check and affected native target passed again. Failed-attempt
logs and the phased coverage explanation remain in the working log.

Focused tests passed six scheduler-withdrawal, four task-withdrawal, and six
strict verifier groups plus freestanding feature Clippy. The final `make check`
passed in 300.59 seconds with 420 default kernel, 463 all-feature kernel, 429
owned-feature kernel, and 100 xtask tests. TCG and KVM runs at 96 MiB and 320 MiB
each accepted 15 records and restored 24,150/81,494 free frames. The seven
feature records are identical across backends. The sole full-transcript
difference is the firmware map's nine TCG versus ten KVM regions, with identical
usable-byte totals. Both proof images have SHA-256
`a8fdfc0206913eb30ba625352c0e47d140da387da0c40d9e7d73aba7afa119b9`.

Normal and workspace images match the prepared-launch and recovery checkpoints.
Peak scope memory was 4.002 GiB under a 6 GiB hard limit and 4 GiB soft limit,
with two build jobs; host available memory stayed at or above 10.697 GiB.
There were 941 soft-limit reclaim events, no hard-limit or scope OOM/OOM-kill
events, and no host OOM kills. No new VirtualBox proof is claimed. The
[Owned Sustained Dispatch Log](../../notebook/Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
records complete evidence, hashes, phase manifests, and diagnostic limits.
This ADR remains Proposed.

## Consequences and remaining limits

This composes two retained launches with bounded sustained execution and
rollback. It does not establish a stable application ABI, general loader,
arbitrary user-copy, general task-local fault policy, TLS/FPU/SIMD ownership,
permanent kernel placement, global W^X, or a production scheduler. The private
gate and exact known fault remain fixture controls. Native Ny, native
Antimatter, and HAG integration are not implemented by this proof.

The next kernel gate is an explicit CPU/context, checked-copy, and task-local
fault profile informed by Ny-generated code and native Antimatter's needs.
Then qualify native launch/startup and application services against that
profile. Antimatter remains the first actual application ABI test; software
reference rendering may precede separately qualified HAG acceleration.

## Alternatives

- Reusing the older fixed roots would leave the retained-launch composition
  unproved.
- Dropping the first scheduler admission after the second fails would lose an
  attached execution context and its cleanup obligation.
- Broadening withdrawal to saved or running contexts would require a separate
  quiescence and suspension contract.
- Reclaiming the faulted owner immediately could clear the active exception
  stack or invalidate a saved continuation.
- Treating all retained failures as retryable would falsely promise recovery
  from failed mapping validation or terminal metadata corruption.
