# Native document open/save proof — 2026-10-04

## Scope and checkpoint

Continue the native Antimatter application contract after the qualified startup
and private-arena checkpoint `529d6b1`. Proposed
[ADR 0056](../../docs/adr/0056-prove-task-bound-native-document-open-and-save.md)
records the experimental boundary. The host prerequisite checkpoint `e3cebf1`
introduces document records, RAM store policy, an opened-editor model and the
production Antimatter save-store reference. Integration checkpoint `990a04c`
adds the native client, kernel profile, artifact/trace gates and an additional
unknown-outcome dirty-state correction. Both checkpoints were pushed before
the final regression matrix completed.

Two independently owned ring-3 tasks run a bounded Rust editor and a combined
RAM document/render service. Four copied routes carry surfaces, input, document
requests and replies. The editor's startup has no seed text: content arrives
through open. Only the service receives initial bytes. It owns open/read/save/
close policy, document versions and handle generations; kernel code retains
identity, memory, route, copy and fault mechanisms.

This proof has one document of at most 31 printable ASCII bytes and one live
handle. Close/reopen preserves committed bytes only while the same RAM service
survives. It does not prove saved data across editor/service destruction or a
fresh pair launch, disk durability, a general filesystem, or full
Antimatter/Electron. Ny remains the official language direction; this bounded
implementation continues in verified Rust.

## Save semantics and native trace

The client keeps current content, a saved-byte baseline and one immutable
pending snapshot separately. A semantic success must match the pending
document, handle generation, request, expected version, editor revision and
exact bytes. It updates the saved baseline, not the current buffer. Transport
acknowledgement only releases copied packet storage and proves no save effect.

The normal trace opens A, selects and replaces `before` with `after` to make B,
submits B, then undoes and inserts `later` to make C before receiving B's reply.
That late reply leaves C dirty. A second save acknowledges C, and close/reopen
returns C under a new handle with a clean new editor state. Eight semantic
surfaces cover these transitions. Reads verify the intermediate and final
committed bytes. Malformed records, retired route/document identities,
stale/closed handles and version conflicts must leave valid progress available.

Service faults before commit and after commit but before reply both leave the
editor uncertain. Its exact pending snapshot stays retained. Unknown outcomes
force unsynchronized/dirty presentation, even if undo restored the last
acknowledged bytes: the service may already hold the pending snapshot instead.
A negative retry response cannot prove the original save did not commit. The
bounded client does not retry an uncertain save. A matching late success can
resolve uncertainty; reopen/reconciliation is a separate explicit action.
Before/after fault probes observe actual RAM store bytes and version before
the distinct nonreturning instruction markers.

A final fresh pair starts a new document identity, new RAM store and zeroed
private arenas. This is ownership cleanup/reuse evidence, not recovery of
saved content from a destroyed service.

## Focused evidence

The parent serialized builds and tests under the existing memory/OOM controls.
The focused editor suite passed 17 tests, including late acknowledgements,
rejection atomicity, edit-away-and-back, and undo-before-unknown. The protocol/
store/startup suite passed 11 tests, and the focused document tooling suite
passed 11 tests. The first focused image build also completed. These receipts
are focused checks, not the final native qualification result.

The host reference invokes the actual Antimatter environment store with a
controlled backend and delayed write completion. It verifies exact saved
snapshots, newer edits, rejected writes and reopened content:

```sh
node scripts/native-document-reference.cjs /path/to/exec-antimatter
```

The reference passed using the existing Linux checkout and dependencies, with
no sibling source edits. Its script and source hashes are retained by the
parent's receipt; this log does not infer a full Electron/native equivalence.

Working receipts are under
an external validation directory (not distributed), including
`focused-results.json`, the focused model/protocol/tooling logs and
`document-reference-qualified.log`. Actual gate counts below come from native
transcripts. Earlier estimated choreography counts remain analysis aids only.

## Native VM and artifact evidence

The native document profile passed TCG/Intel and actual KVM/AMD at both 96 MiB
and 320 MiB, plus independent host-quit captures. Each transcript contains 123
exact records: the 21-record native baseline, readiness, four scenarios,
22 surfaces, 74 document request/reply hashes and final success. The host
constructs expected document wire bytes independently of the service execution
code. Pixel expectations share the editor renderer, so this does not claim an
independent font implementation.

The scenarios run normal document one, failure before commit, failure after
commit before reply, then a fresh normal document two. Each normal run records
276 calls, 42 cooperative switches, eight surfaces, four input messages,
fourteen document requests, thirteen semantic replies, 102 arena copies and
24 published rows. Each fault run records 107 calls, seventeen switches, three
surfaces, two inputs, six requests and four replies. Its delivered save remains
unacknowledged; closure discards exactly that one pending packet. The editor
observes two closed-route results, retains the unknown save, and exits after
the service's contained fault. No fault-run rows are published.

Every pair starts with 65,536 zeroed arena bytes and owns 66 frames (48 data,
18 page-table) and twelve leases. Every terminal path restores all resources
and invalidates the retired authority. The final frame renders
`const later = 2;`; complete TCG/KVM captures match byte for byte. This is the
fresh second document's same-service close/reopen result.

The final TCG/KVM binaries exactly match the independently reviewed artifacts:

| Artifact | SHA-256 |
| --- | --- |
| Native document kernel | `bb76c292fb28715cd3ba540b3bc0a2e7f98965c8553d7bc7390409e6c4289aaa` |
| Native document client | `af05c5a7fa06e34e80b8dc374f01311a5c5975e409970fbb3b9e563cf5999bc4` |

Conservative emitted-code stack bounds are 33,912 bytes synchronously on the
65,536-byte bootstrap stack, or 35,223 including one complete timer path.
Gate/fault/timer bounds, including entry overhead, are 5,911/1,591/1,311 bytes
of their 16,384-byte transition stacks. The complete client bound, including
panic paths, is 4,320 of 8,192 bytes; either service fault-arm site has 2,408
live bytes. Explicit indirect-edge review leaves no unresolved admitted call
or recursion cycle. These bounds apply to these exact binaries and the fixed
sole-BSP profile.

Entry review verifies task CPU capture before ordinary dispatch, integer-only
code before capture and after final restore, exact fault-arm/UD2 sites and
actual volatile observations of the service's RAM bytes/version on both sides
of commit. The kernel remains within Threshold's existing load window and the
2 MiB resident bound. Threshold was unchanged. VirtualBox is installed, but
`/dev/vboxdrv` is not readable or writable by this user; no VirtualBox boot is
claimed.

## Qualification receipt

All **46 qualification checks** passed on the unchanged frozen set of **354
source/configuration files**, followed by both development image restoration
jobs. This covers every repository-required check, earlier native/ELF/Linux/
shared-Veil proofs, Ny, application CPU/copy/fault checks, desktop and workspace
regressions, both actual Antimatter host references, TCG/Intel and actual KVM/AMD
runs, and independent captures. The final host check passes 491 kernel tests
for the native document profile, 531 with all features, 183 tooling tests,
seventeen editor-contract tests, eleven document protocol/store/startup tests
and six existing native runtime tests.

The twelve preceding shared-Veil embedded blobs and the preceding native
runtime RX client blob remain byte-identical. All six earlier ABI profiles
retain equal TCG/KVM whole captures. Normal and workspace development images
were restored and are both byte-identical to checkpoint `529d6b1`'s qualified
images. The new profile remains excluded from ordinary images.

No host or cgroup OOM occurred across any attempt. The final matrix peaked at
4,297,064,448 bytes
(4.002 GiB) in its bounded cgroup,
with at least 17,768,894,464 bytes
(16.549 GiB) host memory available.
It recorded 4,240 soft-limit events, with
zero hard-limit or OOM events. A retained sample near the soft limit was
predominantly file cache; soft throttling did occur. Across all attempts the
bounded cgroup peak was 4,297,064,448
bytes and the lowest host available memory was
17,768,894,464 bytes.

Builds and QEMU jobs ran serially, with two compiler jobs and one test thread.
Limits remained `MemoryHigh=4G`, `MemoryMax=6G` and `MemorySwapMax=512M`; a
two-second monitor stops work for a new OOM event or host available memory
below 4 GiB. No memory-pressure abort was needed. Two earlier focused lint
attempts failed on new manual alignment/range-loop style warnings; both were
repaired before the final source freeze. Their failed receipts are retained.
No VM boot failure occurred in this slice.

| Artifact | SHA-256 |
| --- | --- |
| anomalous.img | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| workspace-dev.img | `bd4a51a6e7a8ab9b6d9e4c1ce4df2112d2c21c85e70eeaa66b26b5e8419266ca` |
| native-document-test.img | `ba0d0eb2d9e635df1a0ec99785a7a4c384b965e2b45e6dfc4e3e097215ebd6a6` |

Evidence is retained under
an external validation directory (not distributed).
`final-receipt.json` verifies source freeze, host counts, all matrix/restoration
results, exact reviewed artifact identity, prior blob/capture comparisons and
the actual Antimatter reference source hashes. `matrix-final-results.json`,
`memory-all-runs.json`, `stack-review.txt`, `critical-entry-review.txt` and the
retained logs/binaries preserve the supporting evidence. Generated binaries,
images and local receipts remain outside Git.

## Next gate

ADR 0056 remains Proposed with this bounded proof qualified; no stable ABI is
claimed. Commands are `make native-document-inspect`,
`make native-document-image` and `make native-document-smoke`.

Next, exercise this same document contract through the qualified ELF/native
personality and compare semantic outcomes and pixels against this native
baseline. A document service surviving editor owner replacement, followed by
scoped filesystem persistence with explicit durability semantics, remains the
later open/edit/save/relaunch gate. Full Antimatter/Electron, threads/TLS,
asynchronous physical input and HAG rendering remain separate work.
