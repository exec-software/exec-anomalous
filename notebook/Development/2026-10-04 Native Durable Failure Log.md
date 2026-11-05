---
title: "Native Durable Failure"
date: 2026-10-04
tags: [antimatter, native-abi, storage, durability, userspace, recovery, faults]
status: bounded_qualification_complete
---

# Native durable failure — 2026-10-04

## Scope and current evidence

This increment extends the initial normal-path
[Native Durable Document Log](2026-10-04%20Native%20Durable%20Document%20Log.md)
under proposed [ADR 0063](../../docs/adr/0063-prove-native-durable-document-storage.md).
It began from `bdd02432334a9bff2590ef030c03286e084a49b0`. The separate
`native_durable_failure_test` profile exercises actual native editor/service
execution, cuts between save operations, loss of a committed save reply, a
newer unsaved edit, and an injected loss of final-flush completion observation.
The old normal durable profile and its version-one fixture remain separate.

The retained native run passed all 42 QEMU processes: 28 status-33 debug-exit
boots and fourteen status-zero host-quit captures. Every boot passed the strict
transcript, independently expected document bytes, and media checks; captures
also passed exact public-region pixel checks. Focused host tests and cross-target
Clippy passed. All 16 required checks, the normal durable proof, native/ELF
replacement and workspace regressions, final binary binding, and an isolated
VirtualBox workspace regression also passed. Implementation checkpoint
`890cb7c` was committed and pushed before the final regression receipt; the
sections below record the completed qualification.

This is the bounded independent Rust client supporting Antimatter's native
runtime direction. It is not the complete Antimatter/Electron application, an
ELF durable comparison, a general filesystem or a stable storage ABI. The
[filesystem architecture](../../docs/architecture/filesystem.md) remains the
larger direction; the five-sector extent and 31-byte text capacity are proof
limits, never native naming/path/depth limits.

## Userspace sequence and failure contract

Each case begins on its own fresh 1 MiB disposable disk. The service explicitly
formats only the blank five-sector grant with empty document A at version one,
then mounts it. Mount commits epoch one before the editor can open. A is not a
host-installed record or an earlier compiled fixture. The host generates a
nonempty attempted save B after building the image, supplies it only to the
initial editor, and retains it independently for verification.

The editor publishes empty A at revision one, inserts B and publishes dirty B
at revision two, then queues a save snapshot containing B. Before the service
consumes that save request, the editor undoes to A and inserts newer C, producing
revision four. C differs from B only in its first byte: increment that printable
byte, or replace `~` with `!`. Both editor and service check C's copied surface
before the service arms its cut. C remains unsaved throughout the proof.

Cases 1–5 terminate the service at an exact admitted `UD2` boundary. No `saved`
reply is delivered. The request was delivered but unacknowledged; closing the
routes discards that one request. The surviving editor observes closure on both
request-acknowledgement and reply directions, with an unchanged rejected-read
destination. It marks the exact B snapshot unknown, retains the pending tuple
and bytes, keeps C dirty, and rejects both another save and a purported definitive
negative resolution. The editor stages its actual C pixels through the narrowly
admitted survivor path, then exits. Full owner/CPU/mapping cleanup follows.

Case 6 completes both required flushes and delivers the delayed `saved(B)` reply
after C already exists. The acknowledgement changes the saved baseline to B,
clears the pending snapshot and leaves C dirty. A subsequent service read returns
B; close succeeds. The service stages C from its independently checked surface,
and both owners exit normally. This case proves that an older acknowledgement
does not mark newer local content clean.

Case 7 actually issues the final save FLUSH, then deliberately exhausts its
remaining completion-observation budget before the first completion sample.
The normal error path poisons the retained device owner. Userspace receives
`outcome_unknown`, quarantines the store and retains its prior acknowledged
version/sequence. Three additional native read/write/flush probes fail without
further port I/O or read-destination changes. The service then takes its admitted
fault, and the editor follows the same unknown/dirty path as cases 1–5. Even after
pair retirement, the poisoned coordinator cannot grant access to another owner.

The expected recovery contract is explicit:

| Case | Native boundary | Permitted recovered content |
| --- | --- | --- |
| 1 | Before executing the save | A, version 1 |
| 2 | Body write completed | A, version 1 |
| 3 | Body flush completed | A, version 1 |
| 4 | Commit write completed, final flush not performed | Complete A/version 1 or B/version 2 |
| 5 | Final flush completed, before reply | B, version 2 |
| 6 | Final flush and delayed reply completed; C remains dirty | B, version 2 |
| 7 | Final FLUSH issued, completion observation lost | Complete A/version 1 or B/version 2 |

No case may recover C, partial text, an invented version, or a checksummed but
unexpected document. These are contained native service failures and an
explicitly injected observation failure. They do not measure a physical device
timeout, abruptly kill the VM during a write, or establish host power-loss safety.
Clean QEMU termination may persist writes that had not passed the guest's final
flush boundary. Cases 4 and 7 therefore retain their old-or-new allowance even
though this run recovered B in both.

## Provisioning, authority and independent cold recovery

The selected failure profile admits canonical 56-byte `ANDI` version two input:
phase zero or two, challenge length/text in the existing fields, and case 1..7
at byte 48. Normal builds still reject version two. The copied 192-byte `ANDD`
version-two descriptor adds the case selector without changing device authority.
The existing canonical mount-ready and document records retain their semantics.
Only the initial editor receives B. The service descriptor has no seed; cold
fixture and recovery descriptors contain no expected text or outcome selector.

Kernel operations 12–14 retain exact current-task, grant, extent, shape and full
copy preflight before PIO. Operation eight arms only the reviewed case while the
exact service is current and the copied-channel state matches the expected
handoff. The kernel observes mechanisms and lifecycle; userspace owns document
parsing, checkpoint selection, saved-snapshot semantics and quarantine policy.
No input number, record checksum or descriptor can mint authority.

After each initial VM exits, the host retains its disk unchanged as pre-cold
evidence and starts a new VM on an exact separate copy. The same inspected code
mounts through fresh owners, advances to epoch two and opens what the disk
actually contains. There is no phase-one in-VM reconstruction in this new
profile; the earlier normal proof covers that separate path. Recovery establishes
current contents/version, not a durable receipt proving the history of the lost
request. A cold emulator restart fences the previous emulator instance under
host cache/flush honesty assumptions; the mechanism offers no in-VM reset.

The host oracle does not execute the guest durable store to decide the answer.
It requires the immutable header, canonical identity/checksums, intact previously
committed A and unchanged outside-extent guards. Unlike the normal-path decoder,
the cut-specific decoder may admit one valid active pair with an invalid inactive
pair. Every valid checkpoint still has to be exactly a known A or B state with
legal epoch/version/sequence. The chosen pre-cold checkpoint determines the
host's cold expectation; guest output never selects that answer. A completed cold
mount must preserve it and advance only the epoch, leaving the former active
pair byte-identical. The retained pre-cold evidence is never mounted or modified.

The runner uses a new writable copy of the inspected boot image for every smoke
attempt, checks it after each VM/capture including error paths, and rechecks the
original image. Data grants remain LBAs `8,13)` on the explicit secondary
`isa-ide` device, separate from the boot disk. Writeback caching has
`cache.no-flush=off,werror=report,rerror=report`; no snapshot mode suppresses data
persistence. The user's running VM and existing disks are not adopted.

## Retained native results

The [smoke receipt (`receipt.json`; not distributed)
retains all fourteen initial/cold pairs and their full pre/post media SHA-256
values. Each of the seven cases ran at 96 MiB and 320 MiB. Each memory size used
a separate fresh data image; its cold launch used a fresh process and an exact
copy of its own pre-cold image. Initial and cold public-frame captures also ran
at 96 MiB for every case, on separate data copies.

| Case | Independently generated B | Initial selected version | Cold epoch/version |
| --- | --- | --- | --- |
| 1 | `cut1-422c0723b7a700cf` | 1, empty A | 2 / 1 |
| 2 | `cut2-1c6483e65e39ba60` | 1, empty A | 2 / 1 |
| 3 | `cut3-13e4b75b6f985854` | 1, empty A | 2 / 1 |
| 4 | `cut4-2601fe32e1151855` | 2, exact B | 2 / 2 |
| 5 | `cut5-2151a89450792417` | 2, exact B | 2 / 2 |
| 6 | `cut6-e07a01d4f7e2e221` | 2, exact B | 2 / 2 |
| 7 | `cut7-77e91260dc0bec4c` | 2, exact B | 2 / 2 |

Both memory sizes produced the same before/after disk hashes within each case.
Initial checkpoint sequence is two when A survives and three when B survives;
cold mount advances it to three or four respectively. The newer C text never
appears as committed content. Initial captures verified C, while cold captures
verified the selected empty A or exact B. Each comparison covers every pixel in
the 384-by-24 public rectangle at `(32,32)`, or 9,216 pixels. It does not claim
whole-screen equality. Host-quit captures are not counted as debug-exit success.

The strict transcript has 41 total records for each initial fault case, 47 for
case 6, or 42 for a cold recovery. Each includes the earlier 21-record native
application baseline. The following counters concern only the new editor/service
pair, not the tasks in that retained baseline:

| Initial case | Calls | Cooperative switches | Arena copies | Completed armed operations | Device commands issued/completed |
| --- | --- | --- | --- | --- | --- |
| 1 | 149 | 17 | 50 | 0 | 20 / 20 |
| 2 | 150 | 17 | 50 | 1 | 21 / 21 |
| 3 | 151 | 17 | 50 | 2 | 22 / 22 |
| 4 | 152 | 17 | 50 | 3 | 23 / 23 |
| 5 | 153 | 17 | 50 | 4 | 24 / 24 |
| 6 | 192 | 24 | 62 | 4 | 24 / 24 |
| 7 | 156 | 17 | 50 | 3 | 24 / 23 |

All initial cases have four authority denials, four bad copies and nine invalid
requests. Fault cases have one service fault, one normal editor exit, two closed
route observations and one discarded request; sends/deliveries are `[3,0,5,5]`
and acknowledgements `[3,0,4,5]` in surface/input/request/reply order. Case 6 has
two normal exits and `[4,0,7,8]` successful sends/deliveries/acknowledgements.
Case 7 records four I/O errors: the injected flush result and three inert probes.
Its device is poisoned and successor grant denial is confirmed; other cases
finish unpoisoned.

Every cold pair has 143 calls, sixteen cooperative switches, 52 arena copies,
five authority denials, four bad copies, nine invalid requests, two normal exits
and `[1,0,6,7]` successful channel operations. Each new pair starts two tasks,
checks 65,536 arena bytes, stages 24 rows and restores its resources. Dynamic
PIT observations and bounded port-read counts are checked separately; they are
not fixed performance claims.

Initial device read/write/completed-flush totals by case are respectively
`10/5/4`, `10/6/4`, `10/6/5`, `10/7/5`, `10/7/6`, `10/7/6`, and `10/7/5`, plus one
IDENTIFY each. Case 7 additionally issued the unobserved final FLUSH. Exact
port-write counts are 1404, 1667, 1670, 1933, 1936, 1936 and 1936. Every cold boot
completes five reads, two writes, two flushes and IDENTIFY: ten commands and 574
port writes. The 224 MiB memory-size difference matches 57,344 frames.

The retained artifact manifest (`manifest.json`; not distributed)
matches independent local SHA-256 readback:

| Artifact | SHA-256 |
| --- | --- |
| Kernel ELF | `98b193fcff5647d0fed63130d86a507872be98a9fbc503343d97b234aaa2b66d` |
| Failure client ELF | `93fb35d28f51aae3838d313b9b115acd45b3505220857973e7263298b9968345` |
| Boot image | `dd6e749ba4f2bcd8b60237f0ddb560a09f0f3ab49043148ba74dcde2dd8661c5` |

`work/native-durable-failure/final-smoke-qemu-processes.json` records all 42 actual
QEMU argument vectors, including Nehalem and explicit backend error reporting.
The receipt binds these results to the retained image; later source formatting
or matrix rebuilds still require final binding rather than assumed identity.

## Focused verification

The retained focused logs/results record 25 passing xtask tests covering normal
and failure artifact/oracle paths; 37 document-protocol unit tests plus ten
portable crash-model tests; and 511 kernel host tests with the failure feature.
Failure-client and kernel freestanding Clippy passed with warnings denied.
The portable crash tests remain modeled evidence, distinct from the actual
native VM cases above. New oracle tests cover exact fixture versions and cold
emptiness, every evidence-field mutation, missing/duplicate/reordered records,
partial final records, permitted media outcomes, torn inactive prefixes,
canonical metadata/guards and cold epoch advancement.

## Full regression matrix and final source binding

All twenty jobs in `work/native-durable-failure/matrix-results.json` passed:
all sixteen repository-required checks, `native-durable-smoke`, both native/ELF
replacement proofs, and `workspace-smoke`. The seven final qualification jobs
also passed: exact failure-artifact rebuilding/binding, kernel all-feature host
and freestanding Clippy checks, normal/workspace image restoration, and the
isolated VirtualBox run plus independent verification.

The first matrix attempt stopped at formatting; its failed log is retained.
Formatting was corrected before the successful full matrix. All 393
frozen non-Markdown source files remained unchanged through qualification.
The formatted source-manifest SHA-256 is
`2af2ee15e8cfac2c49d095ae0f8960e401774b6991bef11e820982a00394b453`.
The rebuilt kernel, client and disk image are each byte-for-byte identical to
the initially exercised and independently reviewed artifacts listed above;
`release-native-artifacts/binding.json` records that final binding. No second
VM run was needed for identical artifacts.

Ordinary boot and workspace images remain byte-identical to the previous checkpoint:
`4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` and
`a9bd1589bdb910f92fea7cd1cf3d5e444e9c3719912e37c8986c505c707e5fa8` respectively.
Current native and ELF replacement image hashes are
`a79632fec48f01aa13081890b774bf7e5113110cf739d74b53a7db0934ec7623` and
`04f6a3fc6b51d182589527abe211216ba0dbd7c632ea1eeda1df4b185af6441f`. They are not wholly identical
to the previous images. The complete image comparison permits only freshly decoded
panic-source line changes and recomputed Threshold checksums: sixteen changed
bytes in native and twenty-two in ELF. Every executable/data/other readonly
byte, client ELF, loader, deployment, plan and blob is unchanged. Boot geometry
and entry are unchanged. The retained
`relaunch-baseline-review/baseline-review.json` carries prior stack certificates
through this exact hash-bound comparison without claiming a new kernel ELF
review. The final rebuilt images match those reviewed copies exactly.

## Exact emitted binary audit

The retained client/kernel review reports bind directly to the kernel and
client hashes above. The client RX payload is 27,564 of 32,768 admitted bytes.
Its complete emitted stack bound, including every panic path, is 5,904 of
8,192 bytes. There are no unresolved indirect calls, recursive cycles or
unreviewed local jump tables. The two distinct UD2 markers and exact startup
and native transport instruction sequences are inspected during packaging.

The kernel's admitted bootstrap envelope is 29,464 bytes, or 30,775 including
one timer entry, within 65,536 bytes. The native gate bound is 6,479 bytes;
fault and timer bounds are 1,591 and 1,311 bytes, each within the 16,384-byte
transition stacks. The reviewer closed 69 manual indirect sites, twelve local
tables and all 53 register-held builtin targets. No emitted stack/control,
CPU-state, ownership or storage-transfer blocker was found. Fatal kernel
panic/Debug formatting is excluded from the admitted-return graph; these
bounds do not certify arbitrary corrupted state or unexpected panic recovery.

The emitted boundary review confirms real FLUSH issuance before completion
observation is removed, permanent transport poison, precise expected service
fault identity/site/IF state, authority revocation before survivor dispatch,
complete copied rows from the surviving editor's actual arena, and full task,
CPU and mapping cleanup. The exact reports and review manifests live in
`work/native-durable-failure/client-review/` and `kernel-review/`.
The final rebuild matches these whole-file hashes; its binding is recorded
with the regression results above.

## VirtualBox regression

A fresh isolated VirtualBox workspace boot observed the expected frame event,
matched all 786,432 pixels against the qualified QEMU initial frame, and powered
off cleanly. The private VM and service remained inside the memory scope. The
user's development VM was not adopted or changed. Receipt and independent
verification are retained in `work/native-durable-failure/vbox-workspace/`.
This qualifies workspace regression only: the native application profile still
safely rejects VirtualBox's masked AMD XSaveErPtr capability, and no native disk
transport result is claimed on VirtualBox.

## OOM and resource qualification

All heavy jobs and VMs were serialized in guarded systemd scopes with two Cargo
workers, one test thread, MemoryHigh 4 GiB, MemoryMax 6 GiB and swap capped at
512 MiB per scope. The guard would stop for host available memory below 4 GiB,
an OOM event, or an increased host OOM-kill counter.

Across eight retained scopes, peak scope use was 4,296,986,624 bytes
(4.002 GiB), and minimum host availability was 20,038,721,536 bytes
(18.66 GiB). The large matrix recorded 4,754 soft-limit events. There were zero
hard-limit, OOM or OOM-kill events, no host OOM-kill increase and no
pressure-triggered abort.
Soft-limit reclamation is recorded separately from failure and OOM. Exact
per-scope samples/results and aggregate `qualification-summary.json` are
retained under `work/native-durable-failure/`.
