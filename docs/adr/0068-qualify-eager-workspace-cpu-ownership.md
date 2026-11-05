# ADR 0068: Qualify eager workspace CPU ownership

- Status: Proposed; implemented and qualified as a bounded single-workspace TCG/KVM proof
- Date: 2026-10-05
- Related: ADR 0048, ADR 0065, ADR 0066, ADR 0067

## Context

The retained workspace in [ADR 0067](0067-qualify-retained-native-workspace-admission.md)
owns its mappings and transition stack, but deliberately uses the earlier
GPR-only, CR0.TS execution contract. The separate native editor and durable
service use eager x87/MMX/SSE2 ownership. Combining them under one coordinator
requires an explicit CPU contract at every transition. Switching between these
contracts implicitly would invalidate the existing state-isolation evidence.

Qualify the real retained workspace with the application CPU owner first. Its
existing desktop, physical-input, fault, watchdog and relaunch paths exercise
the transitions that a later three-task coordinator must preserve.

## Decision

Add the removable `owned_workspace_cpu_test` feature above `owned_workspace_test`.
It does not imply `application_profile_test`: that dependency would also select
the separate dispatcher and suppress workspace entry. Compile the existing
application CPU mechanism for either selected proof, with a narrowly permitted
one-task owner only in this feature. Preserve the GPR-only workspace profile.

Retain one aligned supervisor-only user CPU image, the original kernel image,
and a clean handler image for the whole execution lifetime. The existing CPU
admission restrictions still apply: no XSAVE/AVX, TLS, lazy switching, or unowned
fast system-call entry; unsupported legacy error-pointer behavior is rejected.
The Rust kernel and workspace remain compiled with their soft-float targets.

Use the application's entry and bootstrap-resume boundaries, which do not
toggle TS independently of the CPU owner. Capture the selected user's legacy
state and install clean handler state on the workspace rite, CPL3 timer, input
IRQ, and supported user-fault paths. Restore the exact retained task image
before each ordinary user return. A CPL0 interrupt during the workspace's
integer-only idle interval must not capture kernel state as user state or
restore a user image inside the kernel continuation.

At terminal exit or contained fault, restore the bootstrap root and TSS stack,
restore and verify the original CPU state and controls, and clear the retained
images before releasing the execution owner. Failed provenance or restoration
checks retain ownership and fail closed. The existing framebuffer callback
remains an IF-clear, integer-only bootstrap-root interval with no user copy or
dispatch; it runs while the clean handler image is active.

## Native probe and validation contract

An explicitly selected workspace vessel adds inspected assembly probes. They
observe a clean initial legacy state and retain distinct x87/MMX, XMM and MXCSR
patterns across actual workspace transitions. Probe scratch is bounded private
data, separate from the rite mailbox and admission canaries. Four sealed startup
transitions distinguish stable x87 state, the interval that changes the pattern,
stable MMX state, and completion. Each stable phase requires at least two real
user timer captures. Captures during setup still preserve CPU state, but do not
assert that a partially installed pattern already equals its final value.
No uninspected
compiler-generated SIMD is admitted by relaxing the artifact inspector.

The feature emits strict per-owner CPU preparation and retirement evidence in
addition to the existing workspace lifecycle records. Counters distinguish
rite, timer, input and fault captures; a zero count does not prove that path.
The host oracle must reject malformed, missing, duplicated or out-of-order
evidence and require completed terminal restoration, not infer it from a
displayed frame. A dedicated asynchronous probe must force a user timer
capture; interactive input and the existing terminal cases exercise the real
desktop paths. Inspection and traces must identify the exact final artifacts.

Run the selected native profile at 96 and 320 MiB under an explicit Intel TCG
baseline and actual-host KVM, including the workspace keyboard, pointer,
Graphics, Editor Lab, terminal and relaunch checks. Review the emitted user and
kernel stack paths, gate/IRQ/fault CPU ordering, and resident/boot-loader size
limits. Exercise an isolated VirtualBox initial frame when supported. Run all
repository-required checks and affected CPU/workspace regressions. Rebuild
normal and legacy workspace artifacts and account for any differences.

Serialize builds and VMs with at most two build jobs, a 4 GiB soft / 6 GiB hard
memory scope, and a 512 MiB scope swap limit. The watcher must cover system and
ancestor cgroup pressure as well as available memory and kernel OOM counters:
a userspace pressure-policy kill need not increment those counters. Require a
quiet interval before recovery and use one build job after a pressure incident.
Do not alter unrelated services or host OOM policy. Preserve failed attempts
separately from accepted evidence.

## Qualification result

The frozen profile passed all four 96/320 MiB Intel-baseline TCG and actual-host
KVM configurations: 32 VM boots, 100 complete CPU/execution-owner lifetimes and
2,428 checked interactive frames. All sixteen required targets and affected
workspace/application regressions passed. The normal and both legacy workspace
boot images remain byte-identical to their baselines.

VirtualBox 7.2.6 was exercised, but its AMD guest CPU omits XSaveErPtr and the
existing admission rule rejects it before userspace entry. This is a recorded
capability refusal, not a rendered-frame qualification. No CPUID override or
weakened restoration rule was used.

An initial check scope was killed by systemd-oomd for shared-user-session
pressure despite zero kernel OOM counters. The complete recovery run used
ancestor/system PSI monitoring and one build job; all final scopes completed
without another pressure or OOM incident. Exact artifacts, failed attempts,
stack conditions, memory/journal receipts and remaining limits are in the
[qualification log](../../notebook/Development/2026-10-05%20Eager%20Workspace%20CPU%20Log.md).

## Limits and next integration

This checkpoint does not schedule the workspace, native reference editor and
durable service together. The portable editor bridge remains disabled in native
images. It does not port Antimatter's Electron frontend or broaden the Ny
runtime, foreign ABI, filesystem, display or document contract.

The next coordinator still needs explicit workspace/editor/service roles,
per-route retirement, storage outside the workspace's nearly full user stack,
and cross-task focus fencing. A submitted action must resolve before workspace
publication can change its focus/binding context; the workspace must remain
schedulable to pump the reply. Packet acknowledgement alone is neither action
completion nor a durable-save acknowledgement. Service loss must leave the
workspace and editor able to display newer dirty state and an unknown save.
