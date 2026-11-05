# Mixed native workspace admission

## Scope

[ADR 0070](../../docs/adr/0070-qualify-mixed-native-workspace-admission.md)
continues the native Antimatter path after the portable coordinator in ADR 0069.
The implementation checkpoint uses three small CPL3 probes under the actual
mixed workspace/editor/service mapping profiles. It does not yet run the real
workspace, editor session or durable service together.

The important mechanism is one retained owner for the mixed layouts, one eager
CPU session and scheduler, four exact task-bound routes, and continued
workspace/editor progress after service retirement. The workspace frame region
is nine guarded pages at `0x810000`, sufficient for the two 18,432-byte borrowed
frame slots without adding pixel arrays to its stack.

Baseline source: `b6bfbde`. Design checkpoint: `8e51e5f`.
Mixed mapping checkpoint: `adbcc2a`. Initial native coordinator: `579958a`.
Final retention correction: `a425ebd`; all 431 tracked non-Markdown source files
are frozen for final qualification. These checkpoints are pushed to `origin/main`.
Evidence directory:
an external validation directory (not distributed).

## Memory discipline

Builds and VMs are serialized in task-specific transient units. Each permits
one Cargo build job and one Rust test thread, with 4 GiB MemoryHigh, 6 GiB
MemoryMax and 512 MiB MemorySwapMax. The existing fail-closed guard observes
system, scope and every ancestor's memory pressure once per second, requires
30 continuous quiet seconds before each job, and terminates only its own job
group if pressure, kernel OOM counters, memory availability or monitoring fail.

This turn copies the prior guard into its own evidence directory. The only
instrumentation changes add whether a job was active at each sample and UTC
start/end times in each result. The pure guard self-check passed. No host OOM
policy, unrelated service or user VM is changed. Final scope and journal results are recorded below; kernel OOM counters alone
cannot exclude systemd-oomd intervention.

## Qualification

The first guarded host run passed all 549 kernel tests, including five new
mixed-profile/copy test groups. They reject incompatible geometry, permissions,
missing guards and legacy selection; inject each construction/cleanup clear
failure and partial population failure; and require exact restored accounting.
Six new tooling tests check transcript mutations, CPU/accounting relationships,
feature precedence, complete instruction coverage and read-only placement.

Initial freestanding validation found a shared completion method omitted by its
feature gate and nested unsafe blocks. Later attempts found two lint diagnostics
in the artifact overlap checker and an objdump operand-spacing mismatch. These
were corrected without widening allowed instruction encodings. All attempted
scopes and their failing diagnostics are retained; none is hidden as a pass.

The first linked coordinator passed TCG/Nehalem and KVM/host at both 96 MiB and
320 MiB. Every boot observed three actual CPL3 tasks, 56 gate calls, ten task
selections, six copied/acknowledged packets, six wrong-role denials, five bad-copy
rejections and two closed-route responses. The service faults at its exact UD2
after its queues drain; workspace/editor exchange two further packets, then
both exit. This qualifies quiescent service retirement, not a pending-save or
pending-packet fault scenario.

The initial proof rewrote the workspace arena before its second readback. Review
identified that this could mask corruption caused by a peer at overlapping
virtual addresses. The final source revision removes that refill and
requires every original byte to survive except the exact sent/received words
at offsets 64 and 256. The first matrix therefore establishes admission,
transport, CPU ownership and cleanup, but is not the final arena-retention gate.
The artifact checker also requires exactly one initial REP STOSQ per role; a
seventh tooling test rejects missing/repeated fills with otherwise consistent
bytes, addresses and fault markers. The first V2 lint caught a `usize`/`u64`
loop-offset mismatch; the bounded word index is now explicitly converted before
forming its arena offset. Its failed attempt is retained separately. Final
revision results, artifact hashes and regression outcomes follow below.

The full first-revision `make check` passed. Its scope peaked at 4,296,974,336
bytes and recorded 5,587 MemoryHigh events while staying below MemoryMax. The
minimum observed host available memory was 22,422,441,984 bytes. No kernel/scope
OOM event or pressure-guard abort occurred. The final source passed its separate complete check; this earlier run is
retained as predecessor evidence.

## Final native candidate

Qualification resumed on 2026-10-07 after the final four-cell matrix had
completed on 2026-10-05 (local time). The intervening `90ecc2d` Sync commit
contains documentation only; all 431 frozen non-Markdown source hashes still
match `a425ebd`. No completed native matrix was rerun merely because the chat
resumed.

The corrected image passed all four TCG/Nehalem and KVM/host boots at 96/320 MiB.
Each transcript has exactly 22 structured records and ends with all 144 frames
and 18 leases restored, task authority revoked and the original CPU restored.
Both full arena readbacks passed, checking 73,728 bytes total without a second
fill. TCG observed user timers 3/2/2 and CPU captures/clean checks 64; KVM observed
65/3/2 and 127. Stable checks were 2/2/2 and 2/3/2 respectively; transient
pre-seal timers are counted separately, never confused with canary checks.

The final artifacts are retained under `artifacts-v2`, separately from the
superseded first candidate:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Kernel ELF | 2,753,288 | `c9213109e46334e10be34c95e8b28fbf6e829dfcef1619f5a5feb63c5e7da032` |
| Flat kernel | 245,760 | `8effdcfeb70ec618ac72041856f78d7deb4b6f69b2163d6eb8a880d615b06863` |
| Boot image | 16,777,216 | `2012c174cc6c03a01e3b642f99e13cc1c8a008d3c4f24c09c07c8f40087f0ef4` |

The focused final kernel/tool lint, seven tooling tests and artifact inspection
passed. The four-cell matrix scope peaked at 1,180,598,272 bytes, with no guard
abort or recorded OOM event. Exact artifact hashes were checked against the
matrix outputs before starting final regressions. The final `make check` also passed on 2026-10-07. It peaked at 2,159,415,296
bytes with minimum host available memory 17,688,039,424 bytes and no scope limit,
OOM or guard-abort event. The remaining boot regressions, VirtualBox capability attempt and
memory/journal audit are recorded below.

## Emitted control and stack review

The independent `kernel-review-v2/critical-entry-review.json` receipt binds the
exact final ELF, all 431 frozen source hashes and the consumed analysis scripts
and intermediate records. Its SHA-256 is
`daea4640a6e88d42cb79a9daa23290fbf5184716ff11821b0f840afd19632bbe`.

The closed graph has 181 admitted functions, thirty resolved register-call
sites and eight bounded local tables with 41 entries. All admitted local stack
joins and returns balance; no unresolved call or dynamic stack writer remains.
The eight admitted FP instructions access retained, aligned supervisor images.
Frame ownership precedes capture and dereference; root, TSS, CPU and continuation
restoration precede unpublication and resource release.

| Path | Conservative bytes | Capacity |
| --- | ---: | ---: |
| Native gate | 5,927 | 16,384 |
| Expected exception | 1,655 | 16,384 |
| Timer IRQ | 1,207 | 16,384 |
| Bootstrap plus one conservative IRQ | 31,759 | 65,536 |

The dormant continuation is bounded at 9,968 bytes. The flat kernel leaves
147,456 bytes below Threshold's existing staging limit; resident end `0x194000`
is below the existing 2 MiB bound. No existing stack or staging window was enlarged; the additional workspace
arena uses the explicit mixed profile within existing fixed capacities. Runtime supervisor storage is 29,408 bytes, separate
from the 144 owned frames; no presentation stage is included.

All three fixtures are stackless leaf instruction streams with complete byte and
local-branch coverage. The 1,605-byte workspace has one initial fill and no path
back to it after its first readback. Editor (1,734 bytes) and service (671 bytes)
are byte-identical to their V1 ELF slices. The emitted retention loop covers all
4,608 words on each pass, with only the two exact global-offset exceptions on
pass two and no skipped bytes.

These numeric bounds cover admitted initialization, gates, timer handling,
expected quiescent service fault and cleanup. Arbitrary nested exceptions/NMI
and fatal panic/string-slice diagnostics are outside those bounds. Actual
workspace/editor/service payloads will require a new emitted review.

## Repository regressions

All sixteen required targets passed against the frozen final source on
2026-10-07. Each result retains its command, timestamps, transcript and monitored
scope; builds and guests were serialized.

| Target | Result |
| --- | --- |
| `make check` | Passed |
| `make smoke` | Passed |
| `make fault-smoke` | Passed |
| `make circle-smoke` | Passed |
| `make control-smoke` | Passed |
| `make rite-smoke` | Passed |
| `make scheduler-smoke` | Passed |
| `make supervision-smoke` | Passed |
| `make shell-vessel-smoke` | Passed |
| `make memory-smoke` | Passed |
| `make interrupt-smoke` | Passed |
| `make input-wait-smoke` | Passed |
| `make shell-smoke` | Passed |
| `make semantic-surface-transport-smoke` | Passed |
| `make semantic-presentation-smoke` | Passed |
| `make normal-exclusion` | Passed |

The affected profiles also passed:

- `make application-profile-smoke`: CPU isolation, guarded copies, selected fault
  classes and exact cleanup at 96/320 MiB.
- `make native-interactive-smoke`: physical editing/saves and cold disk recovery.
- `make owned-workspace-smoke` and `make owned-workspace-cpu-smoke`: each verified
  21 keyboard, 465 pointer/Appearance and 26 graphics frames, followed by 95 editor
  frames, with context/fault/watchdog/exit and shell-fallback cleanup.
- `make workspace-smoke`: 512 legacy desktop frames and lifecycle recovery.

## VirtualBox and final audit

The exact final image was attempted with VirtualBox 7.2.6, one CPU and 256 MiB,
using a private configuration, IPC namespace and disk. It reached the kernel,
then emitted `kernel.mixed_workspace_test.failed` with reason
`application_cpu_amd_error_pointers` before any user task was admitted. The AMD
guest CPU lacks the required XSaveErPtr guarantee. No CPU spoofing, admission
weakening or alternate backend concealed this refusal. `vbox-mixed/receipt.json`
retains the failed result and confirmed powered-off cleanup. Host VM cleanup is
not proof of guest task release or successful VirtualBox qualification.

The final audit covers eighteen uniquely named guarded attempts. Initial compile
and lint failures, the corrected retention compile failure and VirtualBox refusal
remain failed attempts; all final required and affected test commands passed.
Every attempted scope has complete results, final memory samples and a stopped
monitor. Recorded job intervals do not overlap. All 431 frozen source hashes and
paths, final artifact hashes, exact commands and available journals were checked.

Across both work periods the maximum scope peak was 4,296,974,336 bytes
(4.002 GiB) and minimum observed host available memory was 17,688,039,424 bytes
(16.473 GiB). Only the predecessor `check` scope recorded MemoryHigh events:
5,587 soft-limit events, with no hard-limit event. No kernel/scope OOM event,
pressure-guard abort or matching kill in accessible unit/oomd journals was
observed. These are sampled and accessible records, not complete system telemetry;
garbage-collected unit counters are explicitly unavailable. Existing host OOM
policies and unrelated workloads were unchanged. No QEMU or VirtualBox process
from the task remained after qualification.

`final-audit/receipt.json` binds the results, source, artifacts, emitted review,
memory records and journal evidence. The four baseline images were rebuilt under
the same guard; their exact byte comparison follows below.

## Baseline image comparison

The normal image is byte-identical to baseline. The three workspace images have
new hashes, with every changed byte independently explained against retained
baseline images and matching ELF LOADs:

| Image | Changed image bytes | Changed kernel bytes | Final SHA-256 |
| --- | ---: | ---: | --- |
| Workspace | 9 | 1 | `8aa7f44f3f3a444e8b5c2124091e39408c4bc1c1c73087719d98c6f236d71408` |
| Owned workspace | 10 | 3 | `7f819e6df5969c4894919215e9fd48f3b6d93743b03505b806ab6eeaa4fc4554` |
| Owned workspace CPU | 11 | 3 | `cc2b16108fac639bb54014fd241837d1a34d35ccf53f11f85e8fa395ac70e0a4` |

Kernel changes are read-only Rust diagnostic Location line fields:
`circle.rs` 207→219 (column 9) in all three, and `prepared_launch.rs` 449→534
(column 17) in the two owned profiles. Source expressions and columns are
unchanged. Other changed image bytes are the correctly recomputed kernel and
manifest FNV32 fields. Threshold stages, executable kernel text, initialized
writable data, embedded workspace payload/data and every other image byte are
identical. This does not claim whole-ELF debug metadata equality.

`workspace-delta/workspace-delta-review.json` binds the retained old/new images,
matching baseline ELFs, source expressions and independent checksum calculation.
Its SHA-256 is `9bba1c5b2ca37fadde99a1bd8e4a2ae9627296fbdc3f4979e895567202817ee5`.
The final compared images and legacy workspace ELF are retained separately from
mutable `build/` outputs. All three affected guest regressions passed independently
of this byte comparison.

## Remaining product work

Attach the real workspace and sole editor session using authenticated copied
observations/results and a local publication fence. Then add durable-service
grants, physical input, composed editor pixels, late-save/service-loss behavior,
cold recovery and complete cleanup to this single runtime. The actual combined
payload and kernel need their own fresh stack and control review. No launcher
entry or full Antimatter/Electron frontend is enabled by the admission probes.
