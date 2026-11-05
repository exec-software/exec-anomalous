---
title: "Owned Multi-region Launch Log"
date: 2026-10-07
tags: [kernel, userspace, memory, abi, linux, qualification]
status: qualified
domain: execution
---

# Owned multi-region launch — 2026-10-07

[ADR 0075](../../docs/adr/0075-prove-owned-multi-region-launch.md) describes this
native prerequisite for unchanged Linux Antimatter. The starting source was
`20a2116` on `main`.

## Native multi-region launch boundary

This increment implements the native memory and entry mechanisms needed to represent the main executable, interpreter and stack inspected in ADR 0074. It does not execute those Linux artifacts. ADR 0073 still sets the application target: run the existing Linux Antimatter build through the Linux veil, preserving its Electron frontend.

The default-off `owned_multi_region_test` feature introduces an explicit nine-region launch profile. It accepts page permissions, reservations, immutable initialization spans, exact executable byte intervals, initial RIP and an explicitly selected guarded stack. Its current operation budgets are 128 user pages, 32 combined page-table pages and sixteen source spans. These are bounded proof choices, not pathname limits or a new stable ABI. Existing fixed launch constructors, circle policies and the default planning capacity remain separate.

Initialization spans must fit wholly inside an admitted region without overlap or arithmetic overflow. Each admitted executable interval must be backed by initialized bytes inside a user RX region. The retained owner revalidates mapping and entry metadata before allocation, requires two distinct initialized RX regions, and publishes no task before explicit commit. Population writes and reads back every region, including zero-filled holes, BSS and page slack. This physical path accepts immutable linked kernel rodata; it does not yet accept a userspace loader's source buffers.

Entry and saved return checks retain the exact executable interval set. They exclude gaps and executable-page padding instead of broadening one range across both images. Hardware permissions remain page-sized: rejecting a padding address as a kernel return target does not prevent a userspace branch into that mapped RX page. The new resident-tree constructor supplies missing low supervisor ancestors while keeping user table indexes and required user permissions intact. Table validation checks exact targets, permissions and absent entries, allowing only appropriate hardware accessed/dirty changes after activation.

Ownership persists through partial population and cleanup. Teardown restores the bootstrap root and privilege stack, withdraws scheduler contexts, unregisters roots, revalidates mappings, retires tasks and circles, clears owned storage and finally removes the physical alias. Failed clears retain unreleased leases and progress for retry. ELF interpretation, dependency lookup and Linux policy remain outside these mechanisms.

## Physical proof

The physical fixture uses two native tasks with main/interpreter-shaped mappings and a 64 KiB user stack each. Each owns 88 user pages, ten table pages and four transition-stack pages: 204 frames, 22 leases and eighteen mapping children in total. These are synthetic GPR-only payloads, not Linux executables.

The final physical evidence covers entry in one RX region, a nested call into the other, checked gates in both regions, and return to the first region by jump. Private canaries and a noninitial stack pointer survived 64 timer switches. An attributed UD2 then faults one task while its peer continues for sixteen timer callbacks before exit. Eight forbidden return sites are admission checks, not hardware fault probes. Two unpublished launches separately exercise transition-stack clear failure and retry. The strict transcript verifier binds ordered lifecycle records and resource restoration to each backend/RAM run. Source and artifact evidence is retained below.

## Next unchanged-Linux boundary

The next concrete target remains the pinned, unmodified `true`, its interpreter and matching libc. The remaining work is to:

- Admit copied or owned loader sources through an authenticated native boundary, then populate the real images and validated Linux initial stack.
- Provide owned mapping, protection and release operations for library loading. This nine-region/128-page fixture cannot itself accommodate libc's inspected 545-page image.
- Establish task-owned TLS and supported CPU state across syscall, interrupt, switch, return and retirement paths; current zero-FS/GS and GPR-only assumptions are insufficient.
- Supply a private filesystem/descriptor view and the startup calls actually requested by the pinned loader. Let that unchanged loader perform relocations, including its supported relocation encodings, and request final protections.

Successful `true` execution would establish a smaller Linux milestone. Electron's processes, threads, synchronization, signals, events and desktop integration remain subsequent requirements for the real Antimatter interface.

## Failures found and corrected

The initial host passes caught a test binding that collided with a page-permission
constant, a fixed-capacity test assumption, unused legacy profile helpers in the
new feature and missing feature-precedence inventory entries. The corrected
focused pass ran 548 kernel and 302 host-tool tests plus lint and image building.
Read-only review also tightened the proof mailbox writer to require a committed
owner and user/RW/NX region before an alias write.

The first two TCG attempts stopped before userspace at the unchanged
`owned_resident_geometry` check. Emitted inspection found excessive simultaneous
bootstrap storage: preparation reserved at least 52,352 bytes, leaving less than
the required 16 KiB. Separating rollback alone was insufficient. Directly
constructing the retained runtime removed duplicate owner storage; admission
identities stay absent until real tasks are committed. The final preparation
checkpoint uses 48,592 bytes, leaving 16,944 bytes of the 64 KiB stack: a 560-byte
surplus above the guard. This is a checkpoint calculation, not a whole-call-tree
bound. Later compiler/source changes require renewed emitted review. The budget
and headroom check were not enlarged or weakened.

The first KVM pass then exposed PIT reinjection debt accumulated during the much
longer IF-clear preparation. A diagnostic pass reported 32 callbacks per task,
progress still 1 for both, and saved RIP `0x60001043`, the ready-gate return.
The tasks had been rescheduled before their user loops could advance. A small
diagnostic compile error was fixed before obtaining this evidence; all attempts
remain retained.

A proof-local one-shot operation now rewrites the existing PIT mode and full
divisor immediately before first entry, after preparation/activation/validation.
Invalid state or repeated use performs no port writes. The helper preserves
PIC masks, IF exclusion, divisor 11932, timebase generation, coarse tick and
normal EOI behavior. A PIC edge already latched may still deliver normally.
The pending-debt interpretation is consistent with
[Linux KVM's timer recreation and reinjection logic](https://github.com/torvalds/linux/blob/v6.17/arch/x86/kvm/i8254.c#L268-L337);
it was tested on the actual host kernel `7.0.0-34-generic`. Both TCG and KVM then
passed without changing the 64-switch, 32/32-quanta or real-progress assertions.
The final focused kernel pass contains 549 passing tests. The change defines no
application timer-control ABI or host-policy workaround.

## Qualification and memory evidence

Evidence root:
an external validation directory (not distributed).
`source-freeze.json` binds 456 tracked and nonignored untracked non-Markdown
source/configuration paths. Implementation checkpoints `db75bbe` and `cb41f68`
were pushed to `origin/main` before the full regression run.

Final physical runs use QEMU 10.2.1 at 96/320 MiB under both TCG and KVM.
All four runs passed the exact 15-record lifecycle oracle, with boot/released
free-frame counts 24,148 and 81,492: the exact 57,344-frame RAM-size delta.
The final TCG and KVM images are byte-identical. VirtualBox 7.2.6 ran that same
image in an isolated 320 MiB VM; its serial transcript passed the independent
strict verifier. The retained harness receipt confirms powered-off cleanup.
The screenshot is only an observation of this kernel proof, not an editor or
pixel-rendering qualification.

Final image SHA-256: `59bde3f9d37fba0da9c158589b3b6b9b4ff7bd9334f25ae257bf41382a51e302`.
Final kernel ELF SHA-256: `5d001a7665cacb7690e23f1086de8baa1e427557f7f89c39d0fc0209426e61c4`.
`emitted-final-review.md` records the exact payload offsets, gate frame layout,
preparation checkpoint, one-shot timer ordering and narrow review limits.

All 24 jobs in the full qualification plan passed: all sixteen AGENTS.md-required
targets, seven affected regressions and a fresh owned-workspace image build.
The affected regressions are `owned-dispatcher-smoke`,
`application-profile-smoke`, `elf-application-smoke`, `linux-application-smoke`,
`shared-veil-smoke`, `elf-relaunch-smoke` and `native-editor-workspace-smoke`.
The normal/native editor/workspace images remain comparison baselines.

The qualification scope ran from 18:59:58 through 19:29:36 UTC. Its peak was
4,297,039,872 bytes (about 4.002 GiB), with at least 10,688,794,624 host-available
bytes (9.95 GiB). The 4 GiB soft limit recorded 5,836 reclaim/throttling events;
the 6 GiB hard limit and OOM counters stayed unchanged. A retained observation
at 19:05:27 UTC showed 3,804,217,344 bytes of file cache and 32,546,816 anonymous
bytes; that snapshot does not attribute every pressure event across the run.
The guard sometimes waited for host pressure to settle before starting another
job. It completed without a pressure abort, stale monitor or runner error.

Builds and VMs ran one at a time with one Cargo build worker and one test thread.
Each scope used a 4 GiB soft limit, 6 GiB hard limit and 512 MiB swap limit;
one-second host, scope and ancestor PSI sampling preceded each job with a
30-second quiet-admission requirement. No host or unrelated process policy was
changed. These sampled counters do not establish continuous system observation.

Baseline comparison retained the original images rather than restoring them over
new builds. The normal boot image and owned-workspace CPU image are byte-identical.
Three other rebuilt images differ and received independent emitted review:

- `workspace-dev.img`: three loaded bytes update two Rust panic source-line
  records, plus image checksums. Code, payload, entry and geometry are identical.
- `owned-workspace-test.img`: the existing resident constructor's sort is now
  inlined; source-location records, code-pointer relocation, NOP padding, entry
  and checksum changes account for the remaining image delta. The constructor's
  fixed 480-byte frame is unchanged and the old 48-byte helper frame disappears.
  Alias, extent, W^X and overlap predicates and the bounded sort remain equivalent.
- `native-editor-workspace-test.img`: the same sort inlining plus source-line
  records, relocated code/data pointers, entry and checksums account for the
  image delta. The native editor workspace regression passed on these exact
  rebuilt bytes. This is not a new numerical bound for its whole call graph.

Final rebuilt image SHA-256 values:

| Image | SHA-256 |
| --- | --- |
| `anomalous.img` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` |
| `workspace-dev.img` | `6e855b537c21399a94cdfba82db3a32173a08c0b32f239914639fed47b2d5eff` |
| `owned-workspace-test.img` | `2aaa8a009e2953feec1d87e442570eca530ba7c24c9d80b3b67aec67eb908033` |
| `owned-workspace-cpu-test.img` | `cc2b16108fac639bb54014fd241837d1a34d35ccf53f11f85e8fa395ac70e0a4` |
| `native-editor-workspace-test.img` | `82ef3ac6d7fa9112e67c5b23f6ef6a7be8163384e0dd249eb164b10be34218d4` |

The read-only `final-audit-v1/receipt.json` passed on 2026-10-07 at 19:39:09 UTC.
Its SHA-256 is
`1dbdb4132f9c6a412562252b70c75908e19aa26aee5f5aa3f2e8c7444400528b`.
It binds thirty selected final jobs across the focused kernel, TCG, KVM,
VirtualBox and full qualification scopes. All 456 frozen non-Markdown paths
remain unchanged. Two baseline images match exactly and three have accepted
hash-bound reviews; no unexplained image changes remain.

The audit retains all sixteen attempted scopes and 54 recorded jobs, including
nine failed development attempts: `focused-initial`, `focused-v2`, `focused-v3`,
`focused-v4`, `physical-tcg`, `stack-v2`, `physical-kvm`, `kvm-diagnostic` and
`kvm-diagnostic-v2`. Their failures and fixes are described above; preliminary
successes do not substitute for the final frozen-source runs. The KVM invocation
log is cumulative and includes earlier failed/diagnostic calls alongside the
final pair. Final image, transcript, emitted-review and baseline-review artifacts
are bound by `final-evidence.json` and the audit receipt.

Across the retained scopes, the peak and minimum available memory are the same
as the full qualification figures above. The maximum sampled PSI avg10 was
1.70% for both some/full; 2,764 samples, including final summary samples, were
retained. Recorded scopes were serialized, with no guard abort or observed
kernel OOM-counter increase. Successful Started journal records bind all
sixteen scopes. Nine retained failed units expose zero OOM-kill counters;
seven collected units have unavailable counters, which are not treated as
historical zeros. The accessible systemd-oomd query returned no entries.
Accessible journals and one-second samples cannot establish complete or
continuous system telemetry.

The resulting qualification is a default-off native memory/entry mechanism
proof. No unchanged Linux executable or Antimatter/Electron process ran in this
increment. The next acceptance remains unchanged `true` with its interpreter
and libc after the loader-source, TLS, mapping and Linux-service gates above.
