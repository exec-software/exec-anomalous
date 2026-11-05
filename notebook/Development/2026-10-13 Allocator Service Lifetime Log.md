# Allocator service lifetime — 2026-10-13

Continue from `18b9583828da6d3528693eb2ade83bc0b575402f`, clean and synchronized
with `origin/main`. ADR 0090 qualified the dormant mapping owner. This increment
removes the bootstrap allocator borrow across guest execution and proves ordinary
allocator reacquisition from the authenticated native service path under
[ADR 0091](../../docs/adr/0091-release-allocator-borrows-before-userspace.md).
Qualification was initially pending and completed on 2026-10-15 as recorded
below. The Linux mapping request remains unanswered.

The user reports HAG is being wired into the Linux Antimatter application. This
reinforces the existing convergence direction: run that actual application through
the Linux veil, then integrate governed compute services in Anomalous. It does
not qualify HAG or Electron on Anomalous or require an editor frontend rewrite.

Evidence is retained at
an external validation directory (not distributed).
The unchanged runner has SHA-256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
Every build, binary analysis and VM run is serial with MemoryHigh 4 GiB,
MemoryMax 6 GiB, swap 512 MiB, one Cargo job, one test thread, the 4 GiB host
available floor and the existing host/ancestor PSI guard. Keep every attempted
scope, including refused or interrupted runs, in the final audit.

HAG explicitly released its preceding native/UI quiet window before this turn.
Its current activity is model-free socket/broker diagnosis; no new GPU run was
queued in that release. Check for renewed coordination before heavy work.

HAG then requested a new quiet window for its corrected native/UI attempt.
No Anomalous build or VM had started. Source implementation, independent review
and proof-tool preparation continued while heavy work remained held for HAG's
explicit release.

HAG reported successful corrected native/UI stages and confirmed workload
retirement and watcher shutdown before releasing the host. Anomalous then
started its first guarded focused scope. That report is coordination context,
not HAG-on-Anomalous qualification.

## Implementation and qualification

Separate non-inlined preparation, entry and teardown phases. The guest-entry
phase holds no allocator borrow or pointer and no broad mutable runtime borrow
across the nonlocal transition. A read-only, kernel-only service probe checks
ordinary acquisition, nested refusal before callback execution, and subsequent
reacquisition while preserving authenticated caller state. The native response
and Linux syscall behavior remain unchanged.

The original focused scope passed eight jobs: formatting, a 490-file source
freeze, 378 xtask tests, 626 kernel tests, both strict lint checks, a fresh image
and its retained capture. The following analysis scope captured the kernel
inventory and bound unchanged native bytes to the preceding native-only review.
Neither scope executed a guest. Fresh kernel emitted acceptance, physical boots,
provisioning refusals, VirtualBox observation and the regression matrix were not
completed before interruption. Commit `86af265` subsequently retained this
implementation with qualification still pending.

## Resume on 2026-10-15

Resume from `5a0c9b1` in the isolated checkout
an external validation directory (not distributed).
Claude is developing the native NIL program launch and console-service route
under ADRs 0092 and 0093. Coordination uses Antimatter's `anomalous` Bot chat
channel through its external inbox. The work is separate from this Linux-loader
profile; it does not widen the earlier native runtime fixture. Heavy builds and
VM runs are serialized across the two efforts.

The allocator runtime and transcript oracle are unchanged from `86af265`, but
the shared ELF adapter, native runtime and other sources have advanced. The old
source freeze cannot qualify this new checkpoint. Fresh evidence belongs in
an external validation directory (not distributed).
The unchanged guard is loaded through a narrow adapter that verifies its hash
and changes only the checkout root. Its memory policy and limits remain the
same; Cargo outputs stay inside the isolated checkout. Historical baseline
image copies remain explicitly identified as historical comparison inputs.

New qualification is in progress. No live mapping publication or additional
Linux return is claimed.

The first resumed focused scope passed formatting, its fresh 558-file freeze,
378 xtask tests and 626 kernel tests, then stopped on
`clippy::manual_range_contains` in the newer native program startup contract.
No image or guest execution followed that failure. Retain `focused-v1` as a
failed attempt. Its peak was 893,575,168 bytes; host OOM kills stayed at the
preexisting count of two and all cgroup OOM/max/high counters remained zero.
At that point, the equivalent inclusive-range check was repaired in this isolated
checkout, with the finding shared through Bot chat. A new source freeze and
focused scope were required after the repair.

`focused-v2` passed all eight jobs on the temporary range-check repair, including
378 host tests, 626 kernel tests, strict lint and image capture. Its peak was
1,809,817,600 bytes, with no new OOM or pressure abort. It did not boot a guest.
Claude meanwhile committed both the equivalent range repair and an unused test
import fix as `14e9083b369c2a909c70fc3ffa61079ac06dc22c`. The local duplicate
was removed and the isolated checkout fast-forwarded to that commit. Retain
v2 as its own successful snapshot, not qualification of the subsequently updated
source. Fresh v3 qualification will bind the committed fixes.

After the completed v2 scope, the heavy-work window was explicitly released to
Claude. Further heavy runs require acknowledgement before either agent starts;
ordinary source editing and lightweight evidence preparation remain independent.

Claude released that window after his vessel build and native runtime smokes.
Resume qualification on the isolated documentation checkpoint `6e30506`, whose
code includes `14e9083`; later main-branch changes are not inputs to this proof.
The ADR 0094 review supports a separate controller-selected arena profile,
preserving independent legacy geometry checks, guard reservations, permissions,
budgets and retryable cleanup. Claude can implement that initial-launch profile
independently; adopting it later requires fresh affected checks. This evidence
does not qualify his future shared-core changes.

`focused-v3` passed the eight focused jobs. The following analysis captured the
fresh kernel inventory, then refused native-review reuse because the current
whole native ELF hash differed. Keep `analysis-v3` as failed. The isolated
checkout also generated a new retained startup random value on its first build,
changing the startup stack and source/plan hashes even though the native code
payload, initialized data, policy, main executable, interpreter and libc hashes
matched. Qualification must explicitly pin the earlier test seed and inspect
the native ELF differences before any reuse of prior emitted-code evidence.
No guest execution followed this mismatch.

The separate `native-diagnostic-v3` scope classified the complete ELF file
delta. Loaded segments, allocated sections including zero-fill geometry, symbols
and regenerated instructions are identical. The section-table offset moves by
32 bytes; only `.debug_info` and `.debug_str` content changes, with the latter
growing by 34 bytes. Subsequent nonallocated sections retain their exact content
at shifted offsets. Every changed raw-file interval is accounted for, including
displaced zero padding. Whole-ELF equality remains false; the failed comparison
is not relabeled as a pass.

`focused-v4` explicitly pins the earlier 16-byte fixture seed as well as libc.
All eight focused jobs pass on `6e30506`, with a new 558-file freeze, 378 xtask
tests, 626 kernel tests, strict lint and retained image. All fifteen deployment
files other than the native ELF and its disassembly are now byte-identical to
the preceding open-file metadata fixture. A separate, independently reviewed
native binding must establish this exact load/deployment equivalence under its
own schema; it cannot reuse an earlier kernel review or physical result.
Fresh kernel inventory capture passes. The thirty required/affected regression
jobs are in progress, still under the unchanged guard.

Claude may run one bounded incremental type check or single-package Clippy
check while this heavy-work window is held, with his explicit memory and time
limits. That allowance excludes tests, image/link builds, `-Zbuild-std` and
guests. Our own guarded jobs remain serial. His ADR 0094 shared-core patch is
being reviewed in the separate primary checkout and is not part of this source
freeze. Review identified a host-test module-gating issue and stale constructor
fixtures before any integration claim.

`qualification-v1` subsequently passed all thirty required/affected jobs,
including the complete `make check`, the boot/runtime regression matrix and
final CPU-workspace image restoration. Its peak was 4,297,019,392 bytes; 4,245
soft-limit events occurred, with no hard-cap event, OOM or pressure abort. Host
OOM kills remained at the preexisting count of two. The complete-scope interim
memory audit retains the failed first focused run and failed whole-ELF analysis.
After the scope and all owned jobs retired, the heavy-work window was released
for Claude's bounded explicit library tests. Our remaining emitted checks and
loader boots wait for that window's release.

## Completed qualification on 2026-10-15

The user subsequently directed us to proceed. Claude had not acknowledged or
claimed the offered test window, and no active compiler or VM was observed.
Bot chat sequence 42 explicitly withdrew that unclaimed offer and reclaimed
the heavy-work window. Our remaining scopes ran serially under the unchanged
guard; the earlier narrow incremental type-check allowance remained separate.

The source qualification remains exactly
`6e305064fffadc6163055609cd8cff3f46f6fc75`, incorporating code baseline `14e9083`.
At final audit, HEAD was `41140ad036f679d0d71f9b97baae9dbc2208b2af`, whose only
changes since that freeze were two Markdown documentation files. The auditor
separately verifies that ancestry and complete current 558-file non-Markdown
inventory. This acceptance update also changes only documentation. Claude's
later shared-core/native-program changes remain outside the qualified source.

`analysis-v4` passes its fresh native diagnostic and distinct schema-2 binding.
All loaded segments, allocated sections, zero-fill geometry, symbols, fault
addresses and instructions match the earlier native-only review; fifteen exact
deployment files also match with the explicit seed. Whole-ELF equality remains
false. The separately reviewed contract permits only the observed nonallocated
metadata case, including the exact raw-byte classification. It establishes no
reuse of previous kernel reviews or physical execution.

`control-v4` passes with 243 reachable functions, no unresolved reachable site
or cycle, all seven stack transfers and the full recoverable fault path. Its
emitted bootstrap bound is 36,816 of 65,536 bytes, leaving 28,720 bytes and
12,336 bytes beyond the required 16 KiB reserve. The largest privileged bound
is 5,792 of 16,384 bytes, leaving 10,592 bytes. These are emitted-code bounds,
not measured runtime stack high-water marks. `semantics-v4` independently passes
eight findings, 228 anchors and 135,168 decoded instruction bytes against the
final kernel. Root approval pins all three reviews and the full artifact manifest
before any new loader boot.

`tcg-v1` and `kvm-v1` each pass smoke, retained capture and exact-artifact
identity. TCG and KVM both pass at 96 and 320 MiB, with 183 records and seven
fresh lifetimes per boot. All 28 lifetimes have two ordinary service borrows,
one refused nested borrow, preserved caller state and restored owned resources.
The strict Rust oracle and separate `physical-crosscheck-v1` pass. Independent
review of all four actual logs also checks 220 reply frames changing only RAX,
28 unchanged terminal frames, all four normal delivery journals and the six
fault-window stage/commit distinctions, peer survival and cleanup witnesses.

Normal execution still makes ten Linux returns and stops at sequence 11,
syscall 9: `mmap(NULL, 2231696, PROT_READ, MAP_PRIVATE | MAP_DENYWRITE, 3, 0)`,
at instruction pointer `0x6002995c`. It receives no reply or Linux exit. The
retained fd 3 offset remains 832 and the service watermark remains 10. These
results establish allocator access, not libc mapping, dynamic linking or
Antimatter/Electron execution.

`negative-v1` passes all six missing, wrong-length and corrupt source/libc
provisioning cases under TCG at 96 MiB. Each rejects before the Linux boundary;
these logs do not expose independent final physical-resource cleanup accounting.
`virtualbox-v1` remains a failed semantic qualification: the exact reviewed image
reports `application_cpu_amd_error_pointers`, then the runner verifies poweroff.
Do not count that capability refusal as a passing loader boot.

The final audit passes 17 completed scopes covering 80 attempted jobs. It retains
the failed `focused-v1`, `analysis-v3` and `virtualbox-v1` outcomes explicitly.
All thirty required/affected regression jobs pass and the CPU workspace image
is restored. All five development images match the explicitly historical
turn-start baseline copies. No owned job intervals overlap. Peak guarded memory
is 4,297,019,392 bytes; minimum sampled host available memory is 13,110,398,976
bytes. The regression scope accounts for all 4,245 soft-limit events; hard-cap,
OOM and OOM-kill counters remain zero in our scopes. Host OOM kills stay at the
preexisting count of two, with no new observed kill or pressure abort. These
claims cover our sampled scopes, complete runner endings and unit journals.

Final evidence remains in the resume directory named above. SHA-256 identities:

| Evidence | SHA-256 |
| --- | --- |
| `source-freeze-v4.json` | `b5cbe9661939c59495c1fc52b7fd45b1628f3d70e463920985d1bae95247e82c` |
| Final `kernel.elf` | `d24cc7bf698c7439afe5e7e257d8b71da7842462abeb1a4f076420fdc6db0dce` |
| `focused-v4-artifact/hashes.json` | `84695437eff01ab15acaa60b72085463d7f240b2702d90cf41e5e0c5ef478c7b` |
| `native-review-crossbinding-v4.json` | `48f6b013cba0dec59814c4a6582493b4c118352b4077eb9949740a400e4047e3` |
| `kernel-emitted-review-v4.json` | `8a23c09ae83e7d86b6ba5f2365e0533250958e2bb4f8379e00bf30b9a656c467` |
| `kernel-emitted-semantics-v4.json` | `cf938761d6dc1ef41d8cd1e2b26faf55d677063611ce982a6d5c640fe5841b81` |
| `reviewed-image-v1.json` | `54701dade9345f4215cce782a569f12dca070d8db0fcc297f1492c56d3bb634f` |
| `physical-crosscheck-v1-receipt.json` | `8d4e0ef463ce7874e4877ca431f99a7327a9cac223b43e904f8401d1d150c4c0` |
| `negative-v1/receipt.json` | `8400d7b9dbd4803dd874f1cfc4df9393537dce5caa50ec515a2033b81bd25035` |
| `vbox-v1-final/receipt.json` | `e8a98d2b099500fd7c01b7f98e310c91487417baac85714722b70f25cbb2f405` |
| `memory-audit-final-v1.json` | `554d471456c1a34d8f09d4f079a6c2fe3f68029ae82cf377ed1c97f75138f6eb` |
| `final-audit-v1.json` | `0c0833cce29b9552df9531b6b45dc224fc53c7a54f6815f8ab5d7e2e90e0d44b` |

Accept ADR 0091 for this bounded scope. The next increment can prepare and cancel
the retained mapping candidate through authenticated suspended-caller authority,
keeping sequence 11 unanswered. It must preserve the requested extent,
partial-page and beyond-EOF behavior, immutable retained source and private
physical access. Combined root, copy, circle, publication, TLB withdrawal and
cleanup ownership remain subsequent integration gates before the mapping reply.
