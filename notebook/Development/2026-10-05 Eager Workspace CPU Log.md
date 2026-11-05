---
title: "Eager workspace CPU qualification"
date: 2026-10-05
status: qualified-bounded-tcg-kvm
domain: userspace
---

# Eager workspace CPU qualification

The next retained-workspace prerequisite is explicit eager x87/MMX/SSE2 CPU
ownership, matching the independent native reference editor and durable service.
[ADR 0068](../../docs/adr/0068-qualify-eager-workspace-cpu-ownership.md) records
the proposed boundary and qualification contract. Base commit: `53e9490`.

Evidence is retained outside the repository at
an external validation directory (not distributed).
The preceding owned-workspace admission evidence remains untouched.

## Plan and limits

- Preserve `owned_workspace_test` as the GPR-only baseline. Select the new CPU
  mechanism explicitly without inheriting the unrelated dispatcher feature.
- Reuse the existing retained application CPU owner and qualify the real
  workspace's rite, idle/IRQ, fault, teardown and relaunch paths.
- Keep native editor bridge pixels off the existing 64 KiB user stack; this
  slice does not enable that bridge or its by-value frame storage.
- Serialize heavy work with the copied memory watcher and a distinct systemd
  unit prefix. Other projects, their processes and the user's VM stay untouched.
- Record actual emitted bounds, CPU probe coverage and cleanup evidence before
  marking this checkpoint qualified. Product Milestone 1 remains open.

## Coordinator findings retained for the next slice

The current two-task native proof assumes `1 - selected` and closes all routes
when either task retires. A three-role coordinator must replace both assumptions.
Four explicit directions suffice: workspace to editor actions, editor to
workspace results/observations, editor to service document requests, and service
to editor replies. Physical input/display belong only to the workspace; the
editor owns the session; the service owns disk access.

Focus safety requires a desktop publication barrier while a submitted action
is unresolved. The workspace must keep running to drain routes. Exact editor
action resolution or trusted terminal route withdrawal can release the barrier;
transport acknowledgement and timeout cannot. A save action may resolve after
reserving its exact snapshot, independently of the later durable reply.

Service retirement closes only document routes and leaves the editor/workspace
route alive. Editor retirement revokes its incident routes and permits explicit
terminal window withdrawal. Workspace retirement revokes new input while the
editor may finish an already submitted save; losing display cannot imply save
success or confirmed close.

## Development attempts

The first tooling run passed 271 host tests. Kernel tests passed, but the first
strict lint rejected a duplicate assembly entry declaration with `*mut` instead
of the existing `*const` frame argument. Importing the canonical application
entry/resume declarations fixed that mismatch. The next artifact inspection
rejected the target's existing `SHF_X86_64_LARGE` text flag; the new inspector
now requires the exact `.entry` and large-model `.text` flags and rejects
unexpected write/TLS/metadata flags. These failed attempts remain in the evidence
folder and are not qualification passes.

The first native candidate is preserved in `artifacts/` with its original
`source-freeze.json`. Kernel ELF SHA-256:
`90ffccacbe3386af0474ed58afe1e9660c13333ccadd72d590fdeb6390b8116f`;
user ELF SHA-256:
`99c76ec874bb35ae93f9cb7af7d8d0d95b844187ba33d349310a94051a9b3d74`.
Its TCG startup stopped at the third CPU probe before reaching the original
workspace canaries, so the later cleanup check reported
`owned_workspace_wide_probes`. Two isolated debugger boots retained the actual
earlier `user_fault` gate rejection: phase two had completed, x87 timer checks
passed, and the third gate carried the exact expected RIP, magic and phase.

The saved image contained the correct MMX 64-bit values, control word, tags and
MXCSR, but retained the earlier x87 exponent bits instead of the required
`ffff` in each aliased register. The installed emulator reports QEMU 10.2.1.
The observed behavior is consistent with its
[MMX entry helper](https://raw.githubusercontent.com/qemu/qemu/v10.2.1/target/i386/tcg/fpu_helper.c)
and [MOVQ translation](https://raw.githubusercontent.com/qemu/qemu/v10.2.1/target/i386/tcg/emit.c.inc):
the former changes TOP/tags and the latter writes the 64-bit payload. This is a
probe setup discrepancy; no CPU owner restoration failure was established.

The correction explicitly seeds all eight 80-bit x87 slots during the permitted
transition phase, then resets the environment and installs the MMX values.
Both user and kernel continue to require the same complete expected register
bytes. The inspector admits only the exact extra FLDT form and its bounded
immutable ten-byte literal. The corrected artifact and its qualification must
be retained separately from this failed candidate.

## Corrected candidate checkpoint

The deterministic exponent setup passed the focused CPU inspector/oracle tests,
all-target xtask lint, image build, and a diagnostic Intel TCG 96 MiB boot. The
boot reached the desktop, matched the previous initial framebuffer exactly, and
retired the guard, NX and desktop owners with restoration evidence. This is a
first-boot checkpoint, not full qualification. The interactive VM matrix, final
emitted-artifact reviews, required regression suite and isolated VirtualBox proof
remain pending.

Accepted candidate inputs are retained under `artifacts-v2/` and
`source-freeze-v2/` in the evidence root. Boot image SHA-256:
`12d2885c2e634492e174b2a822680630035b6be1a2b0c6fbcb7e9e3195b5cde1`.
The corrected RX span is 89,914 bytes. The original failed artifacts and debugger
traces remain separately retained. The candidate-v2 scope peaked at 497,106,944
bytes, with zero soft/hard/OOM events and no host OOM-kill increase.

## Host oracle corrections and emitted review

The first full TCG 96 MiB attempt reached the pointer harness's final
post-withdrawal comparison, which still compared raw CPU records against the
normalized expected transcript. Earlier incremental comparisons had used the
new normalizer. This attempt is retained as `matrix-attempt-1/` and is not a
passing matrix cell. Final verification must normalize and validate the CPU
records while retaining the original raw transcript as evidence.

Independent oracle review also found that the original lower bound on
`user_checks` covered startup but did not account for every later gate. The
strengthened validator binds gates to the completed frame rows, requires checked
gates plus stable-phase timer captures and faults, permits the single unchecked
invalid-mailbox terminal gate, and requires a distinct unchecked watchdog timer.
Impossible counters and arithmetic overflow have explicit rejection tests.
These corrections change host tooling only; the retained v2 guest bytes remain
the review target.

Fresh emitted reviews found no blocker. The user envelope is 61,248/65,536
bytes, with 496 register transfers, 25 local tables and 1,851 direct transfers
closed. Exactly 42 explicit floating-point instructions occur inside the sealed
probe and none elsewhere. Kernel bootstrap plus one interrupt is bounded by
28,247/65,536 bytes; gate plus a nested idle interrupt by 5,294/16,384 bytes.
The kernel review closes 179 admitted functions and all 37 manually resolved
indirect sites, with eight private FX image instructions and 120 boundary
assertions. These are source-conditioned conservative envelopes, not measured
stack high-water values or arbitrary-code verification. Full conditions and
excluded fatal paths are in `user-review-v2/` and `kernel-review/`.

The exact user RX and data, marker aliases, kernel ELF, flat image and Threshold
boot payload are bound together by those reviews. The 348,160-byte kernel
leaves 45,056 bytes in the boot staging limit; resident end `0x1a6000` remains
below 2 MiB. The native editor bridge is still disabled.

The corrected host tooling passed all 277 xtask tests under the guarded required
check, including the new lifecycle counter and final-pointer regressions. All
individual kernel feature test sets and the 561-test combined-feature set had
also passed at this checkpoint. Artifact, lint and native VM qualification were
still running; this is not a full-suite completion claim.

`source-freeze-v3.json` records 424 non-Markdown source files. Its only differences
from v2 are the two host files `owned_workspace_cpu.rs` and
`workspace_pointer_smoke.rs`, recorded in `source-delta-v2-v3.json`; the guest
source assumptions used by both emitted reviews remain exact.

## Shared-session pressure interruption

At 21:14:06 UTC, `systemd-oomd` killed the first required-check scope during
warnings-denied linting. This is an OOM-related interruption and must not be
reported as zero OOM events merely because kernel counters remained zero. The
service journal records 4,297,027,584 bytes peak memory and 64,995,328 bytes peak
scope swap. The last watcher sample still reported about 15 GiB available host
memory, zero kernel/scope hard OOM counters, and 4,535 soft-limit events.

The oomd journal attributes the kill to ancestor `user@1000.service` pressure
of 61.91% above its 50% limit for more than 20 seconds. Fifteen seconds later
it killed the separate `llama-deepseek.service` for pressure in the same
ancestor. This establishes a shared-session pressure event, not its sole cause.
No inference service or host policy was changed by this task.

The terminated runner could not write a final result. Its log, all memory
samples, both journals and an explicit interrupted receipt are retained in
`regressions-attempt-1/`. It is not counted as a completed `make check`. The
watcher originally covered available memory and kernel OOM counters but missed
userspace pressure-policy kills. Recovery adds system and ancestor PSI checks,
uses one build job and preserves the same hard memory/swap limits. The primary
[systemd-oomd documentation](https://raw.githubusercontent.com/systemd/systemd/main/man/systemd-oomd.service.xml)
describes this distinction between PSI-driven userspace action and kernel OOM.

## Required-check recovery and VirtualBox boundary

The revised guard passed pure parsing/threshold checks and a live 30-second
quiet-start exercise. `make check` then completed in 401.47 seconds with one
build job, 1,811,193,856 bytes peak scope memory, 22,387,224,576 bytes minimum
host-available memory and zero soft-limit, hard-limit or kernel OOM events.
All fifteen remaining required targets passed in three separate guarded groups.
The source stayed frozen. Final unit/journal auditing is separate from these
sampled counters.

VirtualBox 7.2.6 was exercised with the exact retained v2 image in a private
256 MiB VM and separate IPC/registration scope. It reached the kernel and
returned the exact fatal reason `application_cpu_amd_error_pointers` before
userspace entry. Its log reports guest `CPUID.80000008:EBX=0x24001000`, with
`XSaveErPtr` absent for the guest and present for the host. The existing AMD
admission rule therefore correctly refuses this exposed capability profile.
The harness powered off its private VM; the user's VM was not changed.

This is an observed VirtualBox capability refusal, not first-frame or interactive
qualification. No CPUID override or relaxed CPU restoration rule was used. The
normal and legacy workspace images still match the baseline, and the independent
TCG/host-KVM matrix remains the selected qualification path for this new profile.
The receipt, serial transcript, guest CPU table and cleanup evidence are under
`vbox-cpu/`; the exact refusal review is retained separately.

## Final qualification

Final source is `source-freeze-v3.json`: 424 non-Markdown files unchanged after
all required and affected checks and the matrix. The source bridge to v2 changes
only the two host oracle files; both guest artifact reviews remain exact. The
first final matrix launch failed before QEMU because moving failed evidence had
removed its recorder's parent directory. A no-exec diagnostic reproduced the
FileNotFoundError; the external wrapper now creates its receipt directory.
`matrix-attempt-2/` preserves that failed launch separately. The accepted run is
`matrix-final-2`, never a merge of incomplete attempts.

| Cell | VM boots | Complete owners | Checked interactive frames | Seconds |
| --- | ---: | ---: | ---: | ---: |
| Intel TCG, 96 MiB | 8 | 25 | 607 | 95.55 |
| Intel TCG, 320 MiB | 8 | 25 | 607 | 95.30 |
| Host KVM, 96 MiB | 8 | 25 | 607 | 83.04 |
| Host KVM, 320 MiB | 8 | 25 | 607 | 83.04 |

Elapsed smoke times describe these runs, not a performance comparison. The
matrix retains raw CPU records and exact backend/memory/image/cgroup receipts.
Every owner has four sealed probe gates, at least two stable x87 and two stable
MMX timer captures, actual clean-handler checks, completed original-control
restoration and cleared image storage. Across the 100 lifetimes, records contain
1,929,040 gates, 11,136 timers, 850 input captures and 72 faults. All 36 desktop
lifetimes observed user-origin input; the 64 guard/NX lifetimes observed zero
input captures. Other IRQ capture counts are zero; no resumed spurious IRQ7/15
coverage is claimed. Transitional pattern setup and terminal watchdog/invalid
return paths explain why not every capture claims a stable-pattern comparison.

There are 2,444 serial frame records; 16 belong to terminal-only probes without
pixel comparison. The remaining 2,428 interactive frames were compared during
the successful smoke runs. Forty-eight selected PPMs are retained and match
across cells. This uses the established shared host/native compositor oracle
plus its independent unit tests; it is not an arbitrary rendering verifier.
`matrix-audit.json` independently closes all retained lifetimes and strict
counter/record/image constraints. SHA-256:
`2c9ade3a51fe31f4fd1f4dfd28e9f13b2a2969fc653be734367eb62459a7db77`.

All sixteen required targets passed, including a complete recovered `make check`.
Affected regressions also passed: GPR-only retained workspace, legacy workspace,
application CPU profile and native interactive editor. After restoration, all
six captured baseline artifacts are byte-identical: normal boot image, legacy
workspace boot image, retained GPR workspace boot image and its user ELF/RX/RW
payloads. `default-artifacts-final/comparison.json` records these comparisons.

The eight final guarded scopes use one build job and the revised PSI monitor.
Their maximum peak is 1,811,193,856 bytes (1.687 GiB), minimum available host
memory is 22,210,756,608 bytes (20.685 GiB), and all sampled scope soft/hard/OOM
counts are zero. All monitored PSI avg10 maxima are zero. The final
`scope-outcome-audit/scope-outcomes.json` binds complete runner receipts and
unit journals; it does not treat garbage-collected unit sentinel counters as
zero or require a nonexistent success-stop log. Completion is inferred from
matching start, completed receipts, final samples and absence of failure/oomd
victim records. Launch-time config hashes were not logged; their audit-time
hashes and exact command/result records are retained. The earlier one oomd kill
of this task remains explicitly outside these successful recovery figures;
there is no whole-turn zero-OOM claim. VirtualBox remains a separate unsupported
capability result, so the audit does not claim every VM proof passed.

## Remaining integration gate

The retained workspace now has a qualified eager CPU contract compatible with
the reference editor/service direction. Next is explicit three-role scheduling
and routing, two 18,432-byte frames in qualified storage outside the nearly full
workspace stack, and the cross-task action/publication barrier described above.
The native editor bridge remains disabled in VM images. This checkpoint does
not put the durable reference editor in a desktop window, port Electron
Antimatter, or complete product Milestone 1.
