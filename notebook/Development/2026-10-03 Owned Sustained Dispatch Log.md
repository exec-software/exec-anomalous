# Owned sustained dispatch

Date: 2026-10-03

## Purpose and scope

The next kernel slice toward native Antimatter composes the retained launch
owner from ADR 0046 with the sustained timer behavior previously proved in
ADR 0039. Proposed
[ADR 0047](../../docs/adr/0047-prove-sustained-dispatch-over-retained-launches.md)
records the architecture. This is a removable two-task GPR-only ownership
proof. It does not establish a stable application ABI, a general loader, or
native Ny/Antimatter readiness.

## Retained owners and rollback

[`owned_execution.rs`](../../kernel/src/arch/x86_64/owned_execution.rs) retains
two `prepared_launch` owners, their circle/task identities, two 16 KiB
supervisor privilege stacks, a checked physical high alias, and root activation
progress. Each launch has the existing RX code, RW+NX data, and guarded RW+NX
user-stack regions. Both launches and privilege stacks together retain 36
frames: 18 data frames and 18 table frames, with ten active and bound leases.
Stack pointers use the supervisor high alias, which remains valid through
private-root switches and bootstrap restoration.

Preparation still creates no task. Explicit commit publishes each ready task
only after verified mapping/payload preparation. The complete source payload
must fit in immutable linked rodata and the admitted code extent. The plan
remains inert and format-neutral; trusted fixture identity metadata is supplied
separately from authority.

[`scheduler.rs`](../../kernel/src/scheduler.rs) adds the narrow
`withdraw_unstarted_context` rollback. It accepts only an exact ready, queued
initial context with consistent task/circle attachments. Any global current
task or dispatch reservation rejects the operation, as do stale or wrong
identities, saved contexts, other states, or corrupt bookkeeping. Queue removal
is precomputed before the final fallible task/circle detach. It preserves the
ring head, all survivors in order, and both sequence cursors.

The corresponding [`task.rs`](../../kernel/src/task.rs) hook detaches the
unstarted context without cancelling the task. The retained launch owner then
uses the earlier `cancel_unstarted` transition before reclaiming its mappings.
The native rollback case uses a real one-slot scheduler: first admission
succeeds, second admission fails for capacity, and the first is withdrawn.
Two injected privilege-stack clear failures retain each remaining lease and
the alias; two subsequent cleanup attempts complete the retained teardown.
Both task/circle generations become stale and allocator accounting returns to
the exact starting snapshot.

Clear/release errors retain progress and permit retry. Failed mapping
revalidation instead retains the owner fail-closed; repeatedly calling
`finish` is not a recovery guarantee. Existing task/circle tombstone failures
also remain terminal. No fallible automatic destructor is added.

## Native execution

[`owned_dispatcher_runtime.rs`](../../kernel/src/arch/x86_64/owned_dispatcher_runtime.rs)
and its [private payload assembly](../../kernel/src/arch/x86_64/owned_dispatcher_entry.S)
drive both committed users through 64 real PIT preemptions, 32 per task. Each
task advances its private counter while register, private-data, and user-stack
canaries remain intact. Nested calls leave each live user RSP at stack top
minus 24 bytes; admitted initialized-code and guarded-stack bounds validate
noninitial returns.

An exact armed task-0 `UD2` produces vector 6 and terminates only that task.
Task 1 continues for 16 sole-runnable ticks, advances its counter without
artificial redispatch, and exits through the private completion gate. The
faulted launch and privilege stack remain retained until both tasks are
terminal. Only then does control return to the separate bootstrap continuation
for complete scheduler, root, mapping, and stack teardown.

The successful totals are 80 timer ticks, 66 dispatches, 64 preemptions, one
fault, and one exit. Final checks require original bootstrap CR3, TSS.RSP0 and
CR0.TS, withdrawn continuations and root registrations, verified storage
clearing, removed alias, and exact allocator/task/circle restoration. No serial
output is added to the hot interrupt path; the final evidence is emitted after
successful cleanup.

## Tooling and strict evidence

The feature `owned_dispatcher_test` depends on `dispatcher_test` and uses the
distinct image `build/owned-dispatcher-test.img`:

```sh
make owned-dispatcher-image
make owned-dispatcher-smoke
cargo xtask verify-owned-dispatcher-transcript build/qemu_owned_dispatcher_96M.log
cargo xtask verify-owned-dispatcher-transcript build/qemu_owned_dispatcher_320M.log
```

The smoke command checks normal-image exclusion, then runs 96 MiB and 320 MiB
sequentially. The strict
[`owned_dispatcher_smoke.rs` verifier](../../tools/xtask/src/owned_dispatcher_smoke.rs)
requires eight boot records and seven ordered feature events: `rollback`,
`ready`, `progress`, `fault_contained`, `survivor`, `released`, and `passed`.
It enforces exact keys, scalar types, values, boot-bound free-frame restoration,
and the 57,344-frame RAM-size delta. QEMU status independently confirms
completion. Mutation tests reject altered fields/types, missing/extra/duplicate
fields and events, wrong order, malformed boot accounting, incorrect size
deltas, wrong process status, and leaked feature markers in normal images.

The new proof is included alongside the existing dispatcher and memory proofs
in the regression workflow. Development feature selectors retain their prior
precedence; the proof remains excluded from normal images.

## Validation

The focused checks passed six scheduler-withdrawal tests, four task-withdrawal
tests, six strict transcript-verifier test groups, and freestanding feature
Clippy. Initial TCG native runs passed at 96 MiB and 320 MiB. Each accepted all
15 records, including the seven feature events, and restored respectively
24,150 and 81,494 free frames.

Review identified and corrected complete linked-rodata source admission and
descriptor/IDT feature selection before the initial source freeze. Independent
read-only review found no actionable scheduler-withdrawal or owned-dispatch
verifier issue.

### Interrupted matrix and scoped harness correction

The first phase passed 15 targets, including `make check` and 14 native targets,
then stopped at the 16th target, `display-vessel-smoke`, when its final transcript
was rejected. No OOM was observed in that phase. The harness removed the rejected
transcript and frame under its existing unverified-artifact policy, so the
specific failure cannot be diagnosed from the original content. Thirty direct
boots of the unchanged kernel subsequently passed; this does not recover the
missing evidence or prove what caused the original rejection.

Independent inspection found a pre-existing capture race: matching the final
event-name substring could trigger monitor capture and quit before that serial
record finished. The narrow correction in
[`tools/xtask/src/main.rs`](../../tools/xtask/src/main.rs) requires a complete
newline-terminated record, exact parsed final-event identity, and the strict
full lifecycle before capture. Two added host tests cover every final-record
prefix, newline handling, and malformed or forged completion. This addresses
the identified race without claiming it was the confirmed cause of the lost
transcript. The corrected predicate and its review fixes were included in the
final revalidation freeze; the complete host check and native display proof
subsequently passed.

The first full host check of that correction caught its positive readiness
case: the expected-event constant still contained a JSON field fragment while
the predicate compared a parsed bare event value. Independent review identified
the same mismatch. The constant was corrected to the bare event name within the
same host file. Failed-check logs are retained. Both focused capture tests and
the complete host check subsequently passed before the corrected phase counted.

Validation used phase-specific freezes of 258 non-documentation files. Only
`tools/xtask/src/main.rs` changed between the initial phase and capture-correction
phase; all kernel and other source/configuration files remained unchanged.
All 258 files in the final freeze stayed unchanged through final validation.
The final phase passed the complete `make check`, `display-vessel-smoke`, and
the ten remaining targets. The earlier 14 native passes were retained for
unchanged target paths and dependencies. This establishes combined coverage;
it is not one uninterrupted matrix on a single unchanged source set.

### Final qualification

All **26 unique targets qualified** in 1,491.42 seconds of summed selected
target time: 14 retained initial native passes plus 12 final-phase targets.
This total excludes failed attempts, the superseded first host check, focused
diagnostics, and final image restoration. The final `make check` passed in
300.59 seconds, including 420 default kernel, 463 all-feature kernel, 429
owned-feature kernel, and 100 xtask tests. The two focused capture-readiness
tests also passed after the event-name correction. The earlier host check
passed in 365.60 seconds with 98 xtask tests, before the two capture tests.

| Evidence | Final result |
| --- | --- |
| Focused ownership and verifier checks | Passed: 6 scheduler, 4 task, 6 verifier groups, freestanding feature Clippy |
| Complete regression coverage | All 26 unique targets qualified across the documented phases |
| Final host check and corrected display capture | Passed: full host check, both focused capture tests, and native display proof |
| Final source freeze | All 258 files unchanged; only `tools/xtask/src/main.rs` differs from the initial phase |
| Owned dispatch under TCG and KVM | Both 96 MiB and 320 MiB passed; 15 records / 7 feature events per run |
| Owned resource restoration | Exactly 24,150 / 81,494 free frames restored on both backends |
| Final normal and workspace images | Byte-identical to prepared-launch and recovery checkpoints |
| Memory and OOM | 4.002 GiB scope peak; at least 10.697 GiB host available; no scope OOM or host OOM kill |

The retained initial native targets are `smoke`, `fault-smoke`, `circle-smoke`,
`control-smoke`, `rite-smoke`, `dispatcher-smoke`, `owned-dispatcher-smoke`,
`scheduler-smoke`, `supervision-smoke`, `shell-vessel-smoke`, `memory-smoke`,
`interrupt-smoke`, `input-wait-smoke`, and `shell-smoke`. The final phase passed
`check`, `display-vessel-smoke`, `supervised-renderer-smoke`,
`semantic-surface-transport-smoke`, `semantic-presentation-smoke`,
`desktop-smoke`, `workspace-smoke`, and `normal-exclusion`, followed by explicit
KVM repeats of memory, dispatcher, owned dispatcher, and workspace smoke.

### Native records and images

Each owned-dispatch run accepted all 15 records, including seven feature events.
TCG and KVM emitted identical feature records and restored the same free-frame
counts at both RAM sizes. The only full-transcript difference was
`boot.memory.map`: nine regions under TCG and ten under KVM. Usable bytes were
identical across backends: 98,918,400 at 96 MiB and 333,799,424 at 320 MiB.
Complete transcripts are therefore not byte-identical.

| Image | SHA-256 | Comparison |
| --- | --- | --- |
| Owned dispatcher proof | `a8fdfc0206913eb30ba625352c0e47d140da387da0c40d9e7d73aba7afa119b9` | TCG and KVM images identical |
| Normal | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` | Identical to prepared-launch and recovery checkpoints |
| Workspace | `65d6cfe39038ac6c078139e9d052a3f790f31eb8b1a018ebd3243cf904251077` | Identical to prepared-launch and recovery checkpoints |

Normal and workspace images were restored in 0.5 seconds each. The existing
KVM workspace remains active in its separate user scope; its unchanged image
needed no restart. No new VirtualBox proof is claimed.

### Resource controls and retained evidence

Heavy targets ran sequentially with two build jobs and `MemoryHigh=4G`,
`MemoryMax=6G`, and `MemorySwapMax=512M`. The watcher retained host/scope memory
measurements and abort conditions throughout the runs. Peak scope memory was
4,296,892,416 bytes (4.00179 GiB), and the minimum host available memory was
11,485,806,592 bytes (10.69699 GiB). The 4 GiB soft ceiling produced 941 reclaim
events, while hard-limit events, scope OOM events, scope OOM kills, and host OOM
kills were all zero. The small peak above the soft ceiling did not reach the
6 GiB hard cap. The separately running workspace retained its own scope.

Evidence is retained under
an external validation directory (not distributed):
`summary.json`, `qualified-results.json`, `final-images.json`,
`native-comparison.json`, `owned-image-hashes.json`,
`source-freeze-before-capture-fix.json`, and `source-freeze.json` record final
coverage and comparisons. Phase manifests/results include `matrix.json`,
`matrix-results.json`, `matrix-resume.json`, `matrix-resume-results.json`,
`matrix-final.json`, and `matrix-final-results.json`, with matching memory
summaries. The preserved `capture-fix-positive-test-failure.log`,
`capture-readiness-tests-final.log`, `check-after-capture-fix.log`, and
`display-race-results.json`, and `display-race-iterations.json` retain correction
and diagnostic evidence alongside
per-target logs. The original rejected display transcript remains unavailable;
these later artifacts do not reconstruct its exact content.

## Next kernel and application gates

Qualify one explicit CPU/context, checked-copy, and task-local fault profile
using Ny-generated code and native Antimatter's requirements. GPR-only fixtures
do not qualify FPU/SIMD or FS/GS/TLS ownership, general memory-copy recovery,
arbitrary exceptions, permanent kernel placement, or global W^X.

Then define and validate native startup/launch and task-bound surface, input,
and workspace services. Native Antimatter remains the first actual application
test of the ABI, beginning with rendered editor content and then exact
open/edit/save/relaunch behavior. Ny needs its freestanding target/runtime;
HAG remains a separately gated native compute transport and acceleration path.
Software reference rendering can precede HAG acceleration.
