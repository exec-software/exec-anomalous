# Mapping Resume Log — 2026-10-18

## Scope and checkpoint

Continue the accepted inactive publication proof at `0ddb673` (qualified code
`924e62c`) in the isolated `codex/runtime-mapping-resume` branch. The target is
the exact first libc mmap reply, followed by observation of the unchanged
interpreter's next request. Linux policy remains in the native veil. Antimatter
continues to mean the existing Linux Electron application, not a native editor
replacement.

[ADR 0097](../../docs/adr/0097-resume-a-caller-with-a-retained-source-mapping.md)
is accepted after physical qualification, baseline review and final audit. The frozen
qualification source is `aa4b409a3ebdfbcca91b46bd89af73ebed8e0795`.
The source separates graft hardware admission, one-use reply
commit, active composed copies, fresh suspended-caller authority and terminal
cleanup. Candidate private ancestry remains exact; only the installed read-only
subtree admits hardware Accessed additions. Original caller entries retain
their existing permission-specific A/D validation.

The fixture rejects premature, altered and replayed completion, including
retirement after publication but before inspection. Both normal retirement and
an eighth lifetime's exact veil UD2 follow the real mapping return and fresh
invocation. Creation handles cannot control the mapping from that invocation.
The complete transcript oracle preserves the six existing file-service fault
lifetimes and distinguishes mmap acknowledgement from file-service watermark.

Source review found and corrected two integration issues before guest execution:
terminal completion cleared the native buffer before checking mapping eligibility,
and the native startup admission still allowed only scenarios zero through six.
Actual next-entry observation, rather than reply commit alone, records that the
interpreter resumed. Reaching that entry does not establish that a candidate
leaf was accessed by hardware.

## Qualification evidence

Evidence directory:
an external validation directory (not distributed).
The earlier `runtime-mapping-publication` evidence remains immutable.

The unchanged memory guard SHA256 is
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
Jobs are serial with one compiler/test thread, MemoryHigh 4 GiB, MemoryMax 6 GiB
and MemorySwapMax 512 MiB. The fresh host boot's OOM counter starts at zero.
HAG and Anomalous explicitly exchange CPU windows; no shared heavy-work window
is assumed from idle-looking process state.

The first guarded preflight passed formatting, all 390 xtask tests, all 651
kernel library tests under `linux_loader_entry_test`, and strict host/kernel
Clippy. The kernel test explicitly uses `--lib`, because its default target
disables automatic library tests. The retained final preflight summary records
peak scope usage of 1,406,963,712 bytes;
no soft-limit, hard-limit or OOM event was recorded.

The preflight source binding is a retrospective parent attestation from the
execution history: no non-documentation source changed between those checks and
the frozen source checkpoint. It is not a contemporaneous pre-test inventory.
The later image, emitted reviews and regression checks use the explicit frozen
inventory directly; `preflight-source-binding-v1.json` records this limitation.

Fresh kernel review covers all 305 functions reachable from the fifteen required
roots, including fault recovery, with no unresolved site or reachable cycle.
The bootstrap bound is 40,208 of 65,536 bytes, leaving 25,328. Maximum privileged
use is 11,200 of 16,384 bytes, leaving 5,184. Two formatter callback cycles are
recorded outside the required roots with uncomputed bounds; no recoverable path
is removed to obtain these bounds. The native owner and crossreview cover all
64 sized functions and 4,782 instructions, with maximum stack use 3,376 of
8,192 bytes. The kernel contributors reviewed their assigned complete bodies;
their authorship scopes are recorded, not described as fully independent of
the implementation. The native crossreview is independent of the helper author.

The current kernel ELF SHA256 is
`4eb4cfc984c7a473f2e7321ab5b5b1daf90699a9e78d0a1f11f48adef5294151`,
native ELF is
`90407539e3742fb9b53d26dbdc85055a97b9d18ec10cff8bd8e871ade229e966`,
and focused disk image is
`2b544849554aa0af43b830dee5cba1df91e7d96fc327d9f91e1c106a39247bc1`.
The full reviewed manifest, source freeze and approval marker also bind the
interpreter, libc, plan, initial stack, native disassembly and runtime fixtures.
Retained and rebuilt TCG/KVM artifacts match those exact inputs.

## Actual mapping return and next request

TCG and KVM at 96/320 MiB each pass eight lifetimes and 234 transcript records.
The first libc mmap returns `0x100000000` through the authenticated native
completion. The unchanged interpreter then reaches sequence 12, which all four
traces identify as Linux x86-64 syscall 9:

```text
mmap(address=0x100028000, length=0x198000, protection=5,
     flags=0x812, descriptor=3, offset=0x28000)
saved RIP=0x6002995c; syscall instruction=0x6002995a; RSP=0x7fff0000e820
```

Protection `5` is `READ | EXEC`; flags `0x812` are
`PRIVATE | FIXED | DENYWRITE`. These values request an executable replacement
over part of the retained libc range. The syscall and flag names were checked
against the host's x86-64 syscall and memory-mapping headers. The proof captures
this new request without implementing its policy or returning it to the caller.
The next slice must address that observed request explicitly in the userspace
veil while preserving the mapping's retained ownership.

Modes 0 and 7 record twelve requests, eleven Linux returns and fifteen context
queries. They make 863 and 862 native calls respectively; the normal lifetime
adds the terminal completion. Each records fourteen lifetime-control calls with
eleven refusals and seven completion refusals. Their service report has active
sequence 12, acknowledgement 11 and file-service watermark 10, with descriptor
3/generation 1 still open at offset 832 and its fence clear. These are distinct
state fields, not interchangeable progress counters.

The six earlier file-service fault lifetimes remain covered. The new eighth
lifetime, scenario 7, reaches the exact linked veil UD2 only after the real mmap
return and fresh request. Its dependent caller and veil retire before the peer
completes, with the full pending frame preserved. Terminal cleanup withdraws
the exact edge, validates the original tree and invalidates translations before
releasing source and mapping ownership. Current and suspended copies use the
exact active owner and fresh request authority; old creation handles remain
invalid during sequence 12.

The independent physical crosscheck binds all four traces, complete before/after
frames, decoded service reports, bytes, geometry and call ledgers. It also
compares otherwise unchanged saved registers across the four runs. All 22
negative trace mutations are refused, including coherent counter adjustments
that try to conceal omitted pathname copies and coordinated saved-frame changes.
Six missing, wrong-length and corrupted source/libc provisioning cases fail
before a caller boundary. The Rust transcript oracle remains a separate check.

`physical-crosscheck-v1-receipt.json` SHA256 is
`1e8e658ac1ac9049c1ab5753b245beda533ee2d10cf7f9f8ac9b00c5a8090240`;
`trace-negative-v1-receipt.json` is
`eb1e5a5379bf4d3569fabd156c325c9092ce2d0418cd6a866279989b62e1577d`;
`negative-v1/receipt.json` is
`95e27b3280f7de4950e33f103eff14a8d5eba8547602ced17c2fa8a9ba7da7ba`.

## Regressions and development baselines

The complete thirty-command union passes: `required-check-v1` supplies
`make check`, and `qualification-v1` supplies the other twenty-nine required and
affected commands. The current VirtualBox 7.2.6 observation at 320 MiB fails
with `application_cpu_amd_error_pointers`, before loader execution, and verifies
poweroff. It remains a separate failed capability observation.

Fresh exact comparisons retain all five 16 MiB images, each contiguous differing
range, a full byte-difference mask and Threshold component/manifest checks.

| Image | Comparison with retained prior baseline | Differing image bytes |
| --- | --- | ---: |
| `anomalous.img` | Byte-identical | 0 |
| `workspace-dev.img` | Byte-identical | 0 |
| `owned-workspace-test.img` | Changed | 163,557 |
| `owned-workspace-cpu-test.img` | Changed | 166,250 |
| `native-editor-workspace-test.img` | Changed | 88,695 |

The three changed images contain broad kernel changes, not merely source
locations or manifest hashes. `baseline-comparison-v1.json` has SHA256
`3aa90dd676212d5f201b02a3106e1bb323e29df56f41ec4f627ecfae568da6e3`.
Its structural location candidates do not establish semantic equivalence.

The source review finds the shared ownership-verification extraction preserves
the original inactive-root, allocator/lease, owner-count and complete-table
checks in the same order. Outside `linux_loader_entry_test`, the current-copy
wrapper retains the original mapping preflight and validation-before-copy path;
the retained graft/admission branch is feature-private. Root and hardware
reviewer authorship is explicit. The hardware reviewer authored the mapping
extraction, so that portion is self-review; its review of the integration/copy
wrapper is independent of that implementation author.

`baseline-source-review-v1.json` has SHA256
`d112a6271fd9591d525d4050f5a399b7307cd7e03094425b3d48b59294c4d49a`.
It does not prove old/new emitted equivalence or transfer the loader profile's
stack bounds to these development profiles. The exact
`native-editor-workspace-smoke` already passes. Image builds and related mapping
smokes did not substitute for the other two exact changed profiles, so root
added `owned-workspace-smoke` and `owned-workspace-cpu-smoke`. Both pass in the
retired `baseline-runtime-v1` scope at 256 MiB. Each checks 607 exact frames:
21 keyboard, 465 pointer/Appearance, 26 graphics and 95 editor frames, together
with controlled faults, watchdog, malformed inputs, orderly exit, resource
restoration and shell fallback. The CPU profile also covers eager CPU ownership.

Root's `baseline-review-v1.json`, SHA256
`98ea9494e7d7bae9d5b16a83fd3328d60d04b83690297f1e6783908fa18fabf8`,
accepts the current changed images under this bounded source/runtime evidence.
It explicitly claims neither old/new binary equivalence nor inherited stack
bounds. These two additional runtime commands supplement the thirty-command
union; they do not change which commands that union contains.

## Retained failed attempts and final resource audit

Six scopes remain retained as failed; successful jobs earlier in a failed
scope retain their own results without relabeling the whole scope as passed:

- `review-diagnostic-v1`: native instruction coverage failed because the manual
  receipt used the wrong padding schema field. The original receipt is retained;
  the exact single NOP was recorded using the required field before rebinding.
- `review-diagnostic-v2`: the native binder passed, then a kernel control witness
  referenced non-instruction address `0x11d4af`. The original contribution is
  retained; the exact EDX instruction anchor is `0x11d4b1`.
- `review-diagnostic-v3`: closure materialization lacked the required rationale
  field. The failed inputs and log remain retained.
- `review-diagnostic-v4`: all-function stack traversal entered a formatter
  callback cycle. Both excluded SCCs were manually attributed; the revised
  analyzer records uncomputed bounds outside required roots and still rejects
  reachable cycles. Diagnostic v5 then passed complete 305-function coverage.
- `review-final-v1`: controls, closure and stack passed; semantic materialization
  failed on a finding-name mismatch. The separate semantics-v2 driver binds
  `allocator_probe_denies_nested_borrow_and_reauthenticates` to required
  `service_borrow_and_nested_refusal` using the same contribution, witnesses and
  range. Both `review-semantics-v1` jobs then passed. The original materializer,
  accepted controls and failed scope remain intact; no repository source changed.
- `vbox-v1`: the current CPU capability refusal above remains failed, with
  verified poweroff recorded by the observation runner.

The host rebooted between the initial scopes and their continuation. First boot
`fa1b0734-4056-45f8-b93e-2904970dfea4` contains preflight, focused image and native
inventory. Continuation boot `799d78fe-4a76-4b21-9fb8-703877c148a2` has its own
recorded baseline. Both begin at `oom_kill 0`; only each scope's own boot-local
delta is checked. Counter resets do not prove that no host OOM occurred between
boots. HAG CPU windows remain explicitly coordinated.

The final audit retains 22 guarded scopes and 75 attempted jobs. All scopes are
retired, with their cgroups checked empty, and no guarded job intervals overlap.
Peak scope memory was 4,297,056,256 bytes; minimum sampled host available memory
was 16,151,531,520 bytes. `required-check-v1` recorded 12,313 `memory.high`
events from soft-limit reclaim. There was no hard-cap event, scope OOM event or
observed boot-local OOM delta, and no scope aborted for memory pressure. These
observations do not fill the gap between the two recorded host boots.

The final audit first wrote a pending receipt under its serial guard, then
sealed after that scope retired and the full memory audit was refreshed. It
binds the exact prepare command and printed receipt hash, current source and
artifact identity, all accepted receipts, failed attempts and baseline reviews.
`final-audit-v1.json` has status `accepted_current_evidence_audit` and SHA256
`5dcf5f6ed6da69613facf81841f1d7181ee86d771ae915fe0a2bf6d35c224039`.
It qualifies source checkpoint `aa4b409`; this documentation update records that
completed evidence without changing the frozen non-documentation source.

This slice proves one actual read-only libc mapping return and interpreter
resumption. It does not prove a hardware Accessed change on a candidate leaf,
implement the requested executable replacement, reach main `true`, finish
dynamic linking, observe Linux exit or run Electron/Antimatter. Same-root caller
reselection also remains outside the bounded contract.

## Parallel native direction

Claude's separate main checkpoint `b49e1b0` qualifies ADR 0094's native
program-bearing mapping/launch at 96/320 MiB. It does not map or execute an
edition. The fixed single-task CPU specialization does not widen launch-derived
mapping admission. This ELF branch intentionally stays on its qualified base;
combining the shared-core changes requires a separate integration qualification.
