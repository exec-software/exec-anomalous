---
title: "Task-owned FS Bases Log"
date: 2026-10-07
tags: [kernel, tls, ownership, native, linux, qualification]
status: active
domain: foundations
---

# Task-owned FS Bases Log

The user authorized the next step toward unchanged Linux Antimatter. This slice
implements native task-owned FS bases under [ADR 0077](../../docs/adr/0077-prove-task-owned-fs-bases.md).
It does not yet run Linux `true`, libc or Antimatter. The separate application
FP/SIMD and Linux SYSCALL profiles still require zero TLS bases.

## Scope and implementation

A small retained session binds FS state to two committed identities. A 72-byte
supervisor capsule captures selectors/bases before entry reloads, establishes
zero kernel bases, and restores a selected FS base after final selector loads.
The 176-byte scheduler frame and existing mapping/stack geometry stay unchanged.
CR4.FSGSBASE remains clear, GS/KERNEL_GS remain zero, and no SWAPGS is used.

Each task makes ten native calls: initial getter, rejected noncanonical and
kernel bases, setting A, getting A, setting B, clearing, getting zero, setting A
again, and getting A. A/B use private high-stack addresses above 32 bits. Actual
FS-relative canaries and progress are corroborated through owned physical
storage. Sixty-four preemptions, a contained UD2 and sixteen survivor callbacks
exercise the lifetime. Explicit retirement precedes bootstrap continuation;
TLS finish/readback precedes mapping release.

The initial design audit suggested reusing two source-derived lifetimes. The
implemented boundary deliberately uses the existing one-lifetime multi-region
pair to isolate CPU-state ownership. Source proofs take feature precedence;
they do not acquire the TLS session. Fresh zero state is covered by host model
tests; a second physical TLS lifetime is not claimed.

## Evidence and memory policy

Local evidence is retained under
an external validation directory (not distributed).
The starting checkpoint was `ed23b3f6c85be51e5f291ac798a104c6177729d1`.
The unchanged memory guard hashes to
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
Every build/test/VM scope uses one build job and one test thread, 4 GiB high /
6 GiB maximum memory and 512 MiB scope swap. The guard requires thirty quiet
seconds, samples host/scope/ancestor PSI once per second, and stops its own
process group at the existing pressure thresholds or below 4 GiB available.
It does not alter host memory policies or other processes. Kernel OOM counters
and transient-unit/journal outcomes are checked separately.

Saved baseline images precede all builds. Source freezes, failed attempts,
final linked artifacts and strict transcripts remain attributable. Final
qualification results will be recorded below after execution.

## Development observations

- Source review caught two pure fixture tests behind a production-only module
  predicate. The functions/tests were extracted into `owned_tls_fixture.rs`
  before the first host run, so the runtime and host tests use identical logic.
- Common assembly hooks are disabled in host-test compilation, where privileged
  helper assembly is intentionally absent. Production hooks are inactive unless
  the selected runtime explicitly acquires the session.

- The first focused run passed 576 kernel tests and 320 host-tool tests, then
  freestanding Clippy rejected a duplicate `allow(dead_code)` attribute on the
  fixture module. Removing the redundant parent attribute fixed it; the second
  focused run passed all eight jobs.
- Independent review found that assigning `None` to optional identity storage
  did not guarantee overwriting its inactive bytes. Before any VM run, the
  owner changed to an explicit twelve-word/96-byte layout. Retirement wipes
  each five-word record, final finish wipes all twelve words, and volatile
  readback checks the cleared storage. A regression checks nonzero identity/base
  bytes and preservation of the other task. The final focused run passed all
  six jobs, including 577 kernel tests and the four freestanding feature mixes.

## Emitted checkpoint before physical runs

Final linked image:
`46ad13555442cd900db26c6e77679e070b0f48bfaf16bece8408d5169a613432`.
The saved ELF is
`0d672f25e9fbbdcd48bede33503e4c5c79d15ad91ebb98918454d55b14773f96`.
Independent review measured the preparation checkpoint as
3520 + 256 + 24400 + 16768 + 3808 = **48,752 bytes**, leaving **16,784 bytes**
on the existing 64 KiB bootstrap stack: **400 bytes above** the unchanged
16 KiB headroom requirement. The rollback chain is 43,056 bytes. Entry helpers
remain small and run on the separate retained transition stacks. The narrow
preparation margin requires renewed emitted inspection after later changes.
The reserve is checked before deeper resident work; those normal helpers consume
1,072 bytes of it, reaching 49,824 total and leaving 15,712 bytes. The largest
normal transition path, including its 176-byte frame, uses 3,376 of the private
16,384-byte stack. Neither stack size nor the admission reserve was widened.

The final integer layout closes the identity-residue finding. Capture precedes
all selector reloads; selected FS restoration follows the last null selector
loads and no later Rust call or selector load occurs before IRET. The emitted
review is separate evidence from the guest runs below.

## Physical qualification

QEMU TCG passed at 96 MiB and 320 MiB using the exact reviewed image above.
Both completed processes produced the strict fifteen-record transcript. Free
frames were 24,145 and 81,489 (difference 57,344), and each restored its initial
count. Each run checked twenty native TLS operations, 64 preemptions, the
contained fault, sixteen survivor callbacks, 106 entries/returns and complete
TLS/resource cleanup. KVM also passed both sizes with identical kernel/image hashes and the same
free-frame counts. Its saved invocation arguments prove hardware acceleration;
VirtualBox passed at 320 MiB with the same image; the isolated harness
observed the completion event and powered off its private VM. The separate
strict transcript verifier accepted all fifteen records. The screenshot is
transport evidence only; this slice makes no rendering claim.

The first implementation checkpoint includes these focused and physical passes.
The full required and affected regression matrix is still pending at that
checkpoint; final acceptance is recorded after those commands finish.

## Next integration boundary

A source audit selects composition with the existing eager-CPU supervised veil
runtime next. Add the same capture/restore ordering to application entry/resume,
INT F4 and Linux SYSCALL after trusted-stack construction. Existing zero-base
checks should remain strict because kernel Rust sees the scrubbed state.
Linux `arch_prctl` decoding and errno policy remain in the userspace veil.

The current setter is deliberately current-task-only. A veil must not use it
to change its own FS on behalf of a blocked caller. The next native mechanism
needs a sealed pending-caller FS query/staged effect, bound to the exact reply
transaction and committed with `prepared_call_reply`. Failed copies, stale or
replayed replies, service faults and retirement must discard the effect. The
acceptance should preserve TLS, eager FP, GPR and red-zone state together.
Use the existing veil's BSS placement for the eager owner; do not spend the
multi-region preparation frame's remaining 400-byte surplus on large CPU state.

That composition still leaves actual image/startup-source integration,
dynamic mappings/protection and Linux file/descriptor/startup services before
unchanged dynamic `true`. Antimatter/Electron follows that real executable
baseline and its broader process/display requirements.

## Required checks

All sixteen AGENTS.md commands passed in the final `qualification-v1` scope:
`make check`, `smoke`, `fault-smoke`, `circle-smoke`, `control-smoke`,
`rite-smoke`, `scheduler-smoke`, `supervision-smoke`, `shell-vessel-smoke`,
`memory-smoke`, `interrupt-smoke`, `input-wait-smoke`, `shell-smoke`,
`semantic-surface-transport-smoke`, `semantic-presentation-smoke` and
`normal-exclusion`. The ten affected regression results and final evidence audit
are recorded after their completion below.

## Completed qualification

The implementation checkpoint `90b3b35` was committed and pushed after focused
and physical qualification, before the broader matrix. All **26** final jobs
then passed on the same **465 unchanged non-Markdown source/configuration
files**. In addition to the sixteen required commands, the affected jobs were
`owned-multi-region-smoke`, `owned-source-smoke`, `owned-dispatcher-smoke`,
`application-profile-smoke`, `elf-application-smoke`, `linux-application-smoke`,
`shared-veil-smoke`, `elf-relaunch-smoke`, `native-editor-workspace-smoke` and
`owned-workspace-image`.

Seven guarded scopes retain all developmental attempts, including the first
lint failure. There were no pressure aborts or observed kernel/scope OOM events.
The final audit below additionally binds transient-unit and accessible journal
outcomes, exact image baselines, all selected jobs and both physical backends.

The final read-only audit passed at `work/owned-tls/final-audit-v1/receipt.json`,
SHA-256 `7f3aa2126b72e553d9fe2ed255ad808276b6b4edfc9fd5beb4de72b5bd5225ab`.
Its manifest is `final-evidence.json`, SHA-256
`5e2b9fbc02fd8ba71a1effcf93bcfe7fe95b609fd59a4042422916c1fdde2610`.
It binds 36 selected final jobs, all seven attempts/47 executed jobs, final
source/artifact reviews and the strict fifteen-record physical transcripts.
All five saved development-image baselines are byte-identical after the suite.

Peak scoped memory was **4,297,048,064 bytes (4.00194 GiB)**; minimum host
available memory was **21,560,147,968 bytes (20.07945 GiB)**. Soft-limit reclaim
occurred, but no hard-limit event, pressure abort or observed host/scope OOM
delta occurred. Collected transient units and unavailable oomd journal history
remain marked unavailable in the receipt; sampled zero counters do not certify
unobservable historical oomd activity. No wider memory-safety claim is made.

ADR 0077 is accepted for this bounded native proof. Current State and execution
readiness now distinguish completed native FS ownership from the still-open
TLS/eager-CPU/Linux-entry composition and unchanged Linux executable target.
