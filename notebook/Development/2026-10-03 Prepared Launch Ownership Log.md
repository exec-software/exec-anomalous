# Prepared launch ownership

Date: 2026-10-03

## Purpose and scope

The user authorized implementation toward native Antimatter, with the editor
serving as the first real application test of the ABI. The first kernel slice
closes retained launch preparation, explicit commit, and cancellation for the
existing owned three-region GPR-only proof.
[ADR 0046](../../docs/adr/0046-prove-retained-prepared-launch-ownership.md)
records the proposed architecture. This is an internal lifetime proof, not yet
an application ABI, Ny runtime, scheduler migration, or running Antimatter.

## Implemented ownership

[`prepared_launch.rs`](../../kernel/src/arch/x86_64/prepared_launch.rs) retains
the mapping stage, entry admission, circle/task identities, attachment progress,
and phase. Preparation validates the exact bounded profile, builds mappings,
requires verified payload initialization, revalidates the tree, and creates
circle/address-space/mapping attachments. It publishes no task. Explicit commit
creates the ready task as its last fallible operation. The immutable load plan
does not supply authentication or authority; principal/origin are still trusted
synthetic fixture metadata supplied separately.

[`task.rs`](../../kernel/src/task.rs) adds `cancel_unstarted`: an exact ready task
with no attached scheduler context can become terminal without pretending to
execute. It retains binding and circle attachment until ordinary destruction.
Other states, stale or wrong identities, and attached contexts reject without
mutation. Cancellation can occur while an unrelated peer is running.

The launch owner checks exact circle attachments before commit or cancellation.
Running tasks and unexpected consumers reject before memory reclamation. After
root/continuation withdrawal, cancellation terminates an unstarted ready task,
releases mappings, detaches resource counts, destroys the task, then destroys
the circle. Each successful step is retained before the next fallible operation.
Failed mapping clear/release remains retryable with all unreleased ownership
reachable. Existing task/circle clearing failures remain terminal tombstones;
the owner becomes poisoned and retains provenance rather than retrying a stale
identity. There is no fallible automatic destructor.

## Native composition

[`user_mapping_test.rs`](../../kernel/src/arch/x86_64/user_mapping_test.rs) now
uses that same owner for preparation, task publication, activation borrowing,
and terminal cleanup. Before running either user, thirteen cases exercise:

- four allocation-initialization failures;
- one failed table write and one failed partial payload initialization;
- one exhausted circle table without disturbing its existing circles;
- one cancellation after successful preparation with no task; and
- five failed cleanup clears after committing an unstarted task, each followed
  by a successful retry.

Every case checks exact allocator/circle/task restoration and the unchanged
bootstrap root. A failed cleanup first makes the task terminal and proves it
cannot start; remaining leases stay in the owner. The subsequent two real users
are then prepared and committed through the same path. B produces two reports
and its contained supervisor-read fault; A produces two reports and exits.
Retained activation borrows prevent owner cancellation while its roots are in
use. Bootstrap restoration and complete session withdrawal precede reclamation
of 28 frames, eight leases, and both task/circle generations.

One final `kernel.memory_test.launch.verified` record follows the existing
`user.released` record and precedes `passed`. The
[`xtask` verifier](../../tools/xtask/src/main.rs) requires its exact ordered
presence, keys, scalar types, failure/retry counts, and booleans. Existing field
and event mutation groups now include it. Normal-image exclusion already bans
the complete `kernel.memory_test.` prefix and now tests the added event too.

## Validation

Focused checks passed: four task cancellation tests, eight retained-owner tests,
seven strict memory-transcript tests, freestanding memory-feature Clippy, and
native memory smoke at 96 MiB and 320 MiB under TCG. The native proof completed
all cancellation cases before the original two-task execution and teardown.
Independent read-only review found no actionable ownership, native-evidence,
transcript, or exclusion issue.

The first full-matrix attempt stopped at the formatting gate because the new
module declaration needed alphabetical placement. That formatting-only change
was applied, then all 254 non-documentation source/configuration files were
frozen for the final matrix. All remained unchanged through validation.

All **24 final targets passed** in 1,059.31 seconds of combined target time:
948.78 seconds for the 21 base checks and 110.53 seconds for the three KVM
repeats. `make check` passed in 275.57 seconds, including 410 default kernel,
453 all-feature kernel, and 92 xtask tests, the broader workspace suites,
formatting, freestanding Clippy, artifact inspection, and feature exclusion.

The remaining base targets were `smoke`, `fault-smoke`, `circle-smoke`,
`control-smoke`, `rite-smoke`, `dispatcher-smoke`, `scheduler-smoke`,
`supervision-smoke`, `shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`,
`input-wait-smoke`, `shell-smoke`, `display-vessel-smoke`,
`supervised-renderer-smoke`, `semantic-surface-transport-smoke`,
`semantic-presentation-smoke`, `desktop-smoke`, `workspace-smoke`, and
`normal-exclusion`. Memory, dispatcher, and workspace were repeated with KVM
required explicitly by the captured QEMU invocation wrapper.

Each memory run at 96 MiB and 320 MiB emitted 24 structured records with 21
selected lifecycle records. Both backends restored exactly 24,148/81,492 free
frames and identical kernel-memory evidence, including thirteen cancellation
cases followed by the two users. Firmware map region counts were nine under
TCG and ten under KVM, with identical usable-byte totals; each passed its strict
boot-map validation. Complete transcripts are therefore not byte-identical.

The captured TCG/KVM memory images are identical. After validation, normal and
workspace development images were restored and compared with recovery:

| Image | SHA-256 | Comparison |
| --- | --- | --- |
| Memory proof | `943f4ba8162db082c50416f14bd991e2bbbc617f13b29ae9deda2bd0ee4d5ed2` | TCG and KVM identical |
| Normal | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` | Identical to recovery |
| Workspace | `65d6cfe39038ac6c078139e9d052a3f790f31eb8b1a018ebd3243cf904251077` | Identical to recovery |

The existing KVM development workspace remains active under its separate 2 GiB
limit; its image is current, so no restart was required. This increment changes
no boot code, and no additional VirtualBox execution is claimed.

Evidence lives under
an external validation directory (not distributed):
`source-freeze.json`, `focused-results.json`, per-command logs,
`matrix-results.json`, `native-comparison.json`, `memory-image-hashes.json`,
`final-images.json`, `summary.json`, and sampled memory/summary files. Earlier recovery
changes remain intact and uncommitted with this increment.

### Resource controls

All builds use two jobs; heavy targets run sequentially in a user scope with
`MemoryHigh=4G`, `MemoryMax=6G`, and `MemorySwapMax=512M`. The watcher samples
every two seconds and terminates only this run's process group if host available
memory drops below 4 GiB or a new host/scope OOM appears. Each command also has
a deadline. The independently running KVM workspace retains its separate 2 GiB
scope limit. No models or GPU compute jobs were launched by this work.

Focused validation peaked at 2,168,995,840 bytes (2.02 GiB), with at least
19,494,137,856 bytes (18.16 GiB) host memory available. Scope memory events were
zero, and the host OOM counter remained zero. Host swap use includes other
development sessions and is not this scope's allocation.

The final matrix peaked at 4,296,667,136 bytes (4.00 GiB), with a minimum of
17,621,471,232 bytes (16.41 GiB) available on the host. The scope recorded 577
soft-limit reclaim events, zero hard-limit events, and zero OOM/OOM-kill events.
The host OOM counter remained zero and the watcher did not abort. Final image
restoration used the same limits, peaked at 30,855,168 bytes, and also passed
without OOM. The soft ceiling is intentionally below the hard ceiling so normal
reclaim can occur without permitting unbounded build growth.

## Next application prerequisite

Combine this owner with sustained timer dispatch while retaining each root,
saved execution context, circle/task identity, and cleanup obligation. Preserve
peer progress through fault and cancellation, then qualify the selected
application CPU-state, user-copy, and fault profile. Ny startup/runtime and
Antimatter surface/input/workspace services must consume that explicit profile.
Antimatter remains the application that will expose whether the ABI supports
useful work; these private UD2 fixtures do not substitute for that test.

A read-only follow-up audit identified the concrete scheduling dependencies:

- `scheduler_table::admit` queues a ready task and attaches its context, but
  current withdrawal requires a terminal context. Partial admission needs exact
  rollback of an unstarted initial context before ordinary launch cancellation.
- Existing dispatcher mailbox/frame helpers use physical addresses as pointers.
  The new low roots require the retained privileged alias for those accesses.
- The current `root_session::select` requires the bootstrap Rust stack, so it
  cannot be called unchanged from an interrupt on a per-task transition stack.
  A scheduling-aware activation owner must retain root registrations and two
  distinct stack leases through the last saved frame and continuation.
- Timer returns must admit initialized code and aligned resumed stack bounds,
  including preemption inside a nested call, instead of requiring the fixture's
  original stack top.

The candidate next proof preserves ADR 0039's 64 real timer switches, exact
fault and 16 survivor ticks over these owned roots, with partial-admission and
stack-cleanup failures. This is planning from source inspection, not additional
implemented or tested behavior in this increment.
