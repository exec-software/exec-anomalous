# Native editor workspace integration — 2026-10-07

The real workspace, remote editor session and durable document service now run
in three private native address spaces and render an editor window in the actual
desktop. The final native matrix, repository regressions, baseline comparison
and source/artifact/resource audit passed.

## Scope and source

[ADR 0072](../../docs/adr/0072-integrate-a-native-editor-workspace.md) connects the
three retained owners through four copied ANEA/ANER and ANDC routes. Only the
workspace has physical input and presentation; only the service has the temporary
sector device. ANEW startup and the qualification-only ANDH delayed-reply marker
have explicit canonical encodings. The editor remains the sole mutable session
and save-snapshot owner. Service loss leaves workspace/editor communication live.

This is a bounded Rust native reference with a 31-byte ASCII line. Full
Electron/React/CodeMirror Antimatter, a general executable launcher, a stable
Reliquary `.re` container, Ny runtime support, HAG and product Milestone 1 remain
open. The public Editor Lab launcher entry remains a separate memory-only app.
A subsequent ELF comparison must preserve these ownership, save, surface and
recovery semantics under an explicitly selected personality.

Protocol/ADR checkpoint: `25140d2`. Integrated runtime/payload checkpoint:
`7770bcdce8b9bf3317c34d8f13988f158d545796`. Final source checkpoint:
`8bc98bb3b31eb20febfdad3ddd91fd4ef8d7f2a3`. All were pushed. Two early GitHub
pushes returned server-side internal errors; the later retry succeeded.

The final correction touches six host verification files: exhaustive selector
exclusions/inventory and an independent glyph assertion. All four guest
artifacts reproduce byte for byte. The final freeze covers 447 non-Markdown
source/configuration files; later qualification notes do not change that source.

## Native result

The final `matrix-v2` run is retained at
`build/native-editor-workspace-proof-6wzvzkjj`. It covers TCG and KVM, each at
96 and 320 MiB, both delayed-reply and lost-reply modes, with initial and cold
boots: 16 boots in eight cases. Physical PS/2 keyboard input and two pointer
motions drive the composed desktop. Cold startup contains no expected text.

- Save B, edit C, receive the late B acknowledgement: C stays dirty. Saving C
  advances the acknowledged baseline and displays clean state.
- Commit B and retire the service before replying: the pending save becomes
  unknown, editing C continues, and explicit close requires confirmation.
  A fresh service in a cold VM recovers B from the same media.
- Complete composition verifies 140 desktop frames and 107,520 rows against an
  independently replayed expected history; 96 stable screen captures match all
  bytes. Transient frames retain exact serial row/readback checks.
- Literal ANEA/ANER/ANDC/ANDH packets, sequence/correlation, disk checkpoints and
  guard sectors are checked independently. An additional read-only review checks
  260 packet records and 5,600 discriminating glyph samples.
- Across the matrix: 121,179 calls, 122,464 CPU captures/clean checks, 208 input
  deliveries, zero input loss, 176 expected role denials, and 48 orderly exits.
  Invalid calls, bad copies, discarded queued packets and user faults are zero.
  Each boot restores all 720 frames, task/circle identities, roots, TSS and CPU
  state through the retained coordinator.

The native image remains opt-in:

```sh
make native-editor-workspace-build
make native-editor-workspace-smoke
```

The default smoke runs the complete matrix. Generated proofs and screenshots
are retained locally, not committed. The final saved-state capture is
an external validation directory (not distributed).

## Exact emitted artifacts and bounds

| Artifact | SHA-256 |
| --- | --- |
| Kernel ELF | `590f39ec45a310392d062af1aa629a9e97742ab46f34247aaabe0e05038a3497` |
| Workspace ELF | `2f5aa6d3bfa61dc13259a1056a20513842a58ebca31c273c480aa1b7c134a137` |
| Shared editor/service ELF | `91525f16f278c70f3a9edf183079b9250a027f392dba30529af9cade2aba6eb2` |
| Boot image | `0d0284832ef85a58c260c341aaa3105c84d68abfa837a6b7923d25e9d5b49acf` |

Workspace RX/data are 102,868/208 bytes; application RX/data are 23,760/4,288
bytes. Their admitted RX capacities remain 131,072 and 32,768 bytes respectively.
The flat kernel is 294,912 bytes within the unchanged 393,216-byte Threshold
capacity. Three owners use 144 frames in 18 leases, including 27 table frames;
the complete desktop stage adds 576 frames in one lease.

| Bound | Conservative emitted bytes | Available bytes |
| --- | ---: | ---: |
| Workspace user stack | 26,704 | 65,536 |
| Shared editor/service user stack | 4,816 | 8,192 |
| Privilege gate plus one idle IRQ | 6,230 | 16,384 |
| Bootstrap plus one IRQ | 32,375 | 65,536 |

The workspace review closes 645 register transfers and 30 local tables with
436 targets. The application has nine closed local tables and no indirect calls.
The kernel review closes 222 functions, 45 manual indirect edges and 11 tables
with 65 targets, including 30 reaching-definition checks. No admitted unresolved
transfer, call-graph cycle, dynamic stack arithmetic or conflicting stack-height
join remains. Bounds exclude fatal panic/invalid-string diagnostics and arbitrary
nested exceptions/NMI; each receipt documents its admitted path assumptions.
The dormant bootstrap continuation is 10,632 bytes.

The CPU review confirms eight explicit FXSAVE64/FXRSTOR64 instructions and no
other compiler-generated privileged SSE/x87/AVX, 18 integer-only preflight
helpers, eight post-restore integer tails, distinct aligned retained images,
root/TSS ordering, capture before policy and terminal restoration. The CPL0 idle
IRQ branch avoids the borrowed runtime and user-capture latch. The final kernel
rebind differs from its prior inspected candidate only in seven payload-extent
LEAs (14 bytes); CPU paths are identical.

## Retained development attempts

Evidence root:
an external validation directory (not distributed).
All failed attempts remain alongside final successes.

- Initial userspace lint found a collapsible conditional. Kernel compile found
  identity accessor, conditional-method and assembly-symbol integration errors.
  Host artifact compilation found a shadowed constant. These were corrected.
- Link inspection rejected a data symbol associated with the preceding read-only
  section; its definition moved inside the writable data section.
- First boot stopped in the strict parser because its expected sequence omitted
  the separate pointer-ready prelude. Exact prelude/mutation checks were added.
- Second boot rejected workspace startup flags: XOR changed arithmetic flags
  before its first gate. MOV now preserves RFLAGS=2, and inspection requires
  those exact entry bytes.
- Third boot rendered the desktop and native window. A later pointer frame
  differed because the host recreated the desktop instead of replaying revision
  history. The independent oracle now replays expected presentation history.
- Review found that an unfocused close-button release lacked focus admission.
  The shared pointer adapter now focuses the native window before requesting
  close, and suppresses close after a rejected transaction, modal or grant denial.
  Host regression groups cover focused/unfocused press/release and rejection.
- A native lint command placed unstable Cargo flags before `clippy`, preventing
  their forwarding. Correcting command order passed without a source change.
- The first full required suite exposed four selector/inventory tests that had
  not included the new lower-priority runtime. Their exact exclusions and known
  development-feature inventory now include it.
- Independent glyph review found one assertion outside its column loop. The
  corrected loop has 70 single-sample mutations spanning seven letter glyphs.
  Host checks passed; the complete final 16-boot matrix passed again.

## VirtualBox

Both isolated 320 MiB VirtualBox 7.2.6 attempts reached the kernel and rejected
`interactive_fixture_signature`: this opt-in profile reads a QEMU fw_cfg fixture.
It did not reach native task startup and is not a qualified application run.
Its CPU report also advertises XSaveErPtr=0, retaining the earlier capability
concern, but fixture admission was the observed failure in this run. No CPU
check was weakened. The final `virtualbox-v2` attempt used the exact qualified
image hash above, and its receipt confirms the owned VM was powered off. The
runner acts only on its isolated VM; unrelated VMs remain untouched. A portable
fixture/device bootstrap is still needed before this profile can run there.

## Repository regression and memory audit

All sixteen repository-required targets passed on frozen source in
`required-v2`: `check`, `smoke`, `fault-smoke`, `circle-smoke`, `control-smoke`,
`rite-smoke`, `scheduler-smoke`, `supervision-smoke`, `shell-vessel-smoke`,
`memory-smoke`, `interrupt-smoke`, `input-wait-smoke`, `shell-smoke`,
`semantic-surface-transport-smoke`, `semantic-presentation-smoke` and
`normal-exclusion`. Focused host codec/history/selector checks, Python mutation
checks, and warnings-denied host/freestanding Clippy also passed.

`regression-v1` passed `mixed-workspace-smoke`, `native-interactive-smoke`,
`owned-workspace-cpu-smoke` and `workspace-smoke`. The desktop result contains
21 exact keyboard, 465 pointer/Appearance and 26 Graphics frames, 512 total.
It also rebuilt `image`, `workspace-image`, `owned-workspace-image` and
`owned-workspace-cpu-image` sequentially for baseline comparison.

All four complete 16 MiB baseline images are byte-for-byte unchanged. Independent
comparison revalidates Threshold manifests, stage-two/kernel checksums and slack;
no diagnostic-byte exception is needed. Retained originals and fresh copies are
bound by `baseline-review/receipt.json`.

| Image | Unchanged SHA-256 |
| --- | --- |
| `build/anomalous.img` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` |
| `build/workspace-dev.img` | `8aa7f44f3f3a444e8b5c2124091e39408c4bc1c1c73087719d98c6f236d71408` |
| `build/owned-workspace-test.img` | `7f819e6df5969c4894919215e9fd48f3b6d93743b03505b806ab6eeaa4fc4554` |
| `build/owned-workspace-cpu-test.img` | `cc2b16108fac639bb54014fd241837d1a34d35ccf53f11f85e8fa395ac70e0a4` |

`final-audit-v2/receipt.json` verifies all 22 attempted build/VM scopes and 69 serialized
jobs, including development failures and both VirtualBox refusals. The successful
final scopes are `matrix-v2`, `required-v2` and `regression-v1`; each starts after
the final source freeze. No frozen source/configuration byte changed. Guest,
baseline, emitted-review and supplemental-provenance hashes are bound together.

Every build/VM scope used one build job and one test thread, 4 GiB memory.high,
6 GiB memory.max, 512 MiB scope swap, a 4 GiB host-available floor and one-second
host/scope/ancestor PSI monitoring, with quiet admission and own-process-group
abort. No host OOM delta, scope OOM/max event, pressure abort or monitor failure
was observed. Maximum PSI avg10 across the observed domains was 1.08%; no sample
crossed the guard's pressure threshold. Host available memory never fell below
18,112,704,512 bytes (16.869 GiB) during monitored scopes.

The overall peak was 4,296,564,736 bytes (4.0015 GiB) in `required-v1`, with 3,041
memory.high reclaim events. This is soft-limit throttling, not a zero-event run.
The successful renewed required suite peaked at 3,159,609,344 bytes (2.9426 GiB),
the final native matrix at 848,797,696 bytes, and the regressions at
1,185,796,096 bytes. Those final scopes recorded no memory.high events.

The final audit retains 66 accessible journal records for the build/VM scopes
and found no attributable OOM/oomd kill. Eleven failed units retain zero OOMKills
and ManagedOOMKills; eleven collected successful units have unavailable
historical counters. The systemd-oomd query returned no accessible records and
no error; this is not complete system coverage. One-second samples also cannot
reconstruct every intersample pressure spike. Guard summaries include a final
sample and distinguish unavailable evidence from a historical zero.

The first read-only audit used a 512 MiB address-space limit, which prevented
one journal mapping despite a zero query exit status. Its original receipt and
stderr remain in `final-audit/`. The final read-only audit instead ran under a
separate 1 GiB memory.high/2 GiB memory.max/128 MiB swap scope; all journal queries
completed without stderr. Neither audit launches a build or VM or changes host
OOM policy. The final receipt is the `final-audit-v2` result listed below.

## Evidence index

All paths below are relative to the evidence root unless explicitly absolute.
The first emitted reviews bind checkpoint `7770bcd`; archived
`source-freeze-7770bcd.json` preserves their recorded hash. The current
`source-freeze.json` binds `8bc98bb`. `source-review-continuity.json` records all
six host-only changes and byte-identical guest rebuilds. The supplemental index
`review-provenance-supplement-8bc98bb.json` resolves the old freeze path without
rewriting the original review manifest.

| Receipt | SHA-256 |
| --- | --- |
| `final-audit-v2/receipt.json` | `f4ac99687106c7bf3ca73d6acaae42a70c92c53a1ad9b765f25bca8f12b6a062` |
| `baseline-review/receipt.json` | `858eb9d4b386494726fe93edb81df632276daebe1f75bdd01555105cd705f5d8` |
| `review-provenance-supplement-8bc98bb.json` | `9063f4c105f9b54cde1641a4f0e1a1dd63223b24da38960f36c8bf38a9731088` |
| `matrix-review-final/receipt.json` | `5660df6b5963a7076d0faf07d8ad6a0a5b6965d5e010e104e9bd0a075fefaa1a` |
| `user-review-final/stack-review.json` | `af29d7f4902594bc423f758c0ad5a4a3d847b459b6b86690eec9db17592bf11b` |
| `application-review-final/stack-review.json` | `e2d55ce54f0e2922d215ac15eff112e3767f17af0de112ece7e3323c3640552e` |
| `kernel-review-final/stack-review.json` | `bd3a2ca9e7ce5594fe3bc4beec50270d92ee3c3af80d4cf9091ed3842bf3828a` |
| `cpu-review-final/cpu-entry-review.json` | `e59f78c046eb59675607b67782dfaac6b22b87158c60c15372afe8dc91503af6` |
| `cpu-review-final/artifact-rebind.json` | `a1485c0f8e9d302c6f326d63eb5f81474e7378c9111b6ddd20bec892ebe05050` |
