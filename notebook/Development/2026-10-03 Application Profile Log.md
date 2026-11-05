---
title: "Application CPU, Copy, and Fault Profile"
date: 2026-10-03
status: complete
---

# Application CPU, copy, and fault profile

[Proposed ADR 0048](../../docs/adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
records the new internal profile above ADR 0047's retained launches. This is
kernel qualification toward native Ny/Antimatter, not an application port or
stable ABI. Existing Rust remains authoritative.

## Source audit and chosen boundary

The active pinned Ny SDK generation is
`456a9a1113d74f6f1c0bcf310cdcd9dc34ead774a584a22770ad7542c4cd676e`, built from
clean source `1ea16a3bbbe84c33a7293cbd828478581d16ab4b` for Linux x86-64 LLVM
Release. Its native runtime archive contains XMM instructions and FS-relative
TLS relocations. Ny emits native floating arithmetic, a host-targeted object,
ordinary C main/startup, and hosted runtime calls. Its compiler/linker lacks an
Anomalous freestanding target/runtime contract. An integer-only fixture would
not qualify this runtime. The current Antimatter frontend uses Electron,
React, CodeMirror and DOM rendering; its native frontend/services remain open.

The selected increment therefore owns legacy x87/SSE2 state, bounds copied
user-memory operations and classifies selected task faults. TLS, AVX/XSAVE,
threads, SMP and general untrusted application admission stay outside it.
SYSENTER/SYSCALL entry is required disabled. Unsupported CPU states fail closed.
The ADR links the Intel/AMD specifications and Linux's AMD error-pointer quirk.

## Implementation and observed invariants

- The CPU session owns four aligned 512-byte images: original kernel, two users
  and clean handler state. Eager capture loads clean state before Rust dispatch;
  every user return restores its selected image. The final fault restores and
  independently compares defined kernel state, restores exact CR0/CR4/EFER,
  then volatile-clears/verifies the images. AMD/Hygon require XSaveErPtr.
- User copies resolve the exact current retained task/circle generations and
  root/TSS stack. A pure plan bounds operations to 1..4096 bytes. Preflight
  checks all touched live translation paths against retained leases and
  permissions before constructing short-lived supervisor-alias spans. Returned
  rejection precedes every destination write; privileged hardware faults remain
  fatal. No user virtual pointer is directly dereferenced.
- The fault classifier handles CPL3 #DE/#UD/#GP/#PF without fixture instruction
  locations. It rejects privileged, reserved and unsupported error profiles.
  Return admission applies to the survivor; terminal RIP/RSP may be invalid.
  `fault_last_current` retains the last attached context, clears current and
  consumes no sequence. Reclamation follows bootstrap continuation restoration.
- Four successive pairs each complete 64 preemptions, 32 per user, with all
  sixteen XMM registers, x87 state/control and MXCSR preserved, then 16 survivor
  ticks. First faults are divide, invalid opcode, general protection and page
  fault. The page fault is an instruction fetch at absent RIP `0x802000`.
  Each last task uses UD2 with RSP zero and returns to bootstrap for cleanup.
- The private fixture clears saved IF only at an exactly armed terminal return.
  This prevents a timer in the deliberate invalid-stack/fault instruction
  window. Ordinary selected frames and the surviving peer keep timer delivery.
  This exception is fixture control, not general user authority to suppress
  preemption. Fault policy itself remains separate from fixture observations.
- Each pair performs five successful checked copies and ten rejections.
  Odd cross-page data/stack round trips and RX reads succeed. Wrong-current and
  genuinely retired identities, overflow, invalid tails, guards, zero length
  and code writes reject. Complete kernel destination sentinels and independent
  both-owner data/stack/code readbacks detect mutation. Host tests additionally
  cover oversized/supervisor/noncanonical ranges and corrupted live PTE paths.
- Task/circle tables are new per pair, with a real canceled/destroyed identity
  before launch-slot reuse. This proves stale generation rejection in those
  tables; it does not claim one persistent table across all four pairs.

## Preliminary validation and corrections

The first kernel host compile found a test using `NonZero<u64>` as an integer;
adding `.get()` fixed the test. An intermediate native Clippy run encountered
an in-progress CPU-vendor evidence field and shared old-proof helpers unused
under the new feature; the field and narrow feature guards were completed.
The subsequent initial TCG proof passed both sizes before review closed the
terminal IF timing window. These development attempts are preserved, not
counted as final frozen-source qualification.

After that correction, 449 application-feature kernel host tests, 106 xtask
tests and freestanding feature Clippy passed. The final-source initial KVM
proof passed both sizes using the actual AMD host CPU, restoring 24,146 and
81,490 free frames. Its image SHA-256 was
`88274ac9716bc04291a198be2b4e3ae15050d42af165709414d59baa55513bb0`.
A source freeze covers 265 non-documentation files. The first full check reached all host tests and native inspections, then Clippy
rejected a redundant return wrapper in the new smoke harness. Removing the
wrapper changed only `tools/xtask/src/application_profile_smoke.rs`; standalone
xtask Clippy then passed. `source-freeze-before-lint.json` retains the original
manifest and `source-freeze.json` covers the final 265 files. A fresh full
28-target matrix then passed; no earlier partial matrix result is counted as
final qualification.

## Evidence locations and resource controls

Evidence lives outside the repository at
an external validation directory (not distributed):
`source-freeze.json`, `focused-final-results.json`, `matrix.json`, result and
memory files, per-command logs, preserved TCG/KVM serial transcripts and images.
The KVM wrapper changes only the backend, and substitutes `host` for the new
proof's explicit TCG `Nehalem` model. Its invocation log preserves actual args.
CPU vendor is reported and strictly admitted in the new transcript schema.

Builds run sequentially with two Cargo jobs in a user scope capped at 4 GiB
soft / 6 GiB hard and 512 MiB scope swap. A two-second watcher terminates only
this job's process group on new OOM or host available memory below 4 GiB.
Final resource totals, source comparison, hashes and regression results are
recorded below. No new VirtualBox execution is claimed while KVM
continues owning host virtualization; kernel modules/other workloads were not
reconfigured.

## Final frozen-source qualification

All **28 targets passed** in **1,153.82 seconds** of summed target time. The
fresh full `make check` passed in 227.05 seconds: 440 default kernel tests,
483 all-feature kernel tests, 449 application-profile kernel tests, and 106
xtask tests, plus the required inspections, combined-feature links and Clippy.
The new host coverage comprises three CPU-image/admission groups, two fault
classifier groups, four last-current scheduler groups, five copy-plan/binding
groups, four retained live-page-path groups and two alias byte-copy groups.
All 265 final frozen non-documentation files remained unchanged. The failed
initial full check is retained separately and contributes no final target pass.

The strict native oracle accepted 16 records at each RAM size on both TCG
(`Nehalem`, GenuineIntel) and KVM (`host`, AuthenticAMD). Each boot completed
four pairs/eight task lifetimes, 320 timer observations, 264 dispatches,
256 preemptions, eight task-local faults, 20 successful copies and 40 rejected
copies, with verified state restoration and cleanup. Both backends restored
24,146 free frames at 96 MiB and 81,490 at 320 MiB, an exact 57,344-frame delta.
Their eight feature records differ only in CPU vendor. The boot memory map
contains eight TCG versus ten KVM regions, with identical usable-byte totals
at each RAM size. Both proof images have the same SHA-256 shown below.

Normal and workspace images were rebuilt after qualification and match the
recovery/prepared-launch/owned-dispatch checkpoints byte for byte. The older
owned-dispatch image has a new hash after the shared-owner changes; its TCG
and KVM regression targets pass. No new VirtualBox proof is claimed.

The final matrix peaked at **1.984 GiB**;
its host available memory stayed at or above **21.600 GiB**.
It recorded zero soft-limit reclaim, hard-limit, OOM or OOM-kill events.
Across all development/validation attempts, peak scope memory was
**4.002 GiB**; the initial failed host check recorded
501 soft-limit reclaim events, with no hard-limit event. Scope OOM and host
OOM-kill counters stayed zero across every recorded phase. Two build jobs,
4 GiB soft / 6 GiB hard limits and the watcher remained in use.

| Image | SHA-256 |
| --- | --- |
| `anomalous.img` | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| `workspace-dev.img` | `65d6cfe39038ac6c078139e9d052a3f790f31eb8b1a018ebd3243cf904251077` |
| `owned-dispatcher-test.img` | `7d8939da7b101a38a0564b9f2b618ecc27bdab7fb199eb545de5a204dd10aede` |
| `application-profile-test.img` | `88274ac9716bc04291a198be2b4e3ae15050d42af165709414d59baa55513bb0` |


| Target | Backend | Seconds | Result |
| --- | --- | ---: | --- |
| `check` | host | 227.05 | Passed |
| `application-profile-smoke` | tcg | 36.01 | Passed |
| `smoke` | tcg | 1.50 | Passed |
| `fault-smoke` | tcg | 1.50 | Passed |
| `circle-smoke` | tcg | 30.51 | Passed |
| `control-smoke` | tcg | 31.51 | Passed |
| `rite-smoke` | tcg | 31.01 | Passed |
| `dispatcher-smoke` | tcg | 30.51 | Passed |
| `owned-dispatcher-smoke` | tcg | 31.51 | Passed |
| `scheduler-smoke` | tcg | 31.01 | Passed |
| `supervision-smoke` | tcg | 31.51 | Passed |
| `shell-vessel-smoke` | tcg | 32.01 | Passed |
| `memory-smoke` | tcg | 33.01 | Passed |
| `interrupt-smoke` | tcg | 31.01 | Passed |
| `input-wait-smoke` | tcg | 32.01 | Passed |
| `shell-smoke` | tcg | 1.00 | Passed |
| `display-vessel-smoke` | tcg | 31.01 | Passed |
| `supervised-renderer-smoke` | tcg | 33.01 | Passed |
| `semantic-surface-transport-smoke` | tcg | 74.52 | Passed |
| `semantic-presentation-smoke` | tcg | 75.52 | Passed |
| `desktop-smoke` | tcg | 47.51 | Passed |
| `workspace-smoke` | tcg | 71.52 | Passed |
| `normal-exclusion` | host | 29.01 | Passed |
| `kvm-memory-smoke` | kvm | 32.01 | Passed |
| `kvm-dispatcher-smoke` | kvm | 30.51 | Passed |
| `kvm-owned-dispatcher-smoke` | kvm | 31.51 | Passed |
| `kvm-workspace-smoke` | kvm | 48.51 | Passed |
| `kvm-application-profile-smoke` | kvm | 36.51 | Passed |


Final evidence is in `matrix-final-results.json`,
`matrix-final-memory-summary.json`, `source-freeze.json`, `summary.json`,
`tcg-profile-96M.log`, `tcg-profile-320M.log`, `kvm-profile-96M.log`,
`kvm-profile-320M.log`, and both retained proof images under the evidence
location above. `kernel-target.json` records the pinned kernel target's
SIMD-disabled soft-float features, no-red-zone setting and abort strategy.
The source/document review checked 486 local Markdown targets across eleven
documents with no missing paths; two historical/current wording ambiguities
were corrected. Development image restoration passed separately.

## Remaining work

The next bounded gate is one inspected freestanding Ny-generated object with a
qualified startup/runtime shim and explicit instruction/relocation/import
contract. Do not link the existing hosted `libnyruntime.a` and call it native.
Then qualify copied surface/input/workspace services and the native Antimatter
frontend through rendering, editing and exact open/save/relaunch behavior.
Software reference rendering can precede HAG device/compute transport.
#MF/#XM, debug and other unselected fault classes still need explicit policy
before broader external-program admission; this slice does not close them.
