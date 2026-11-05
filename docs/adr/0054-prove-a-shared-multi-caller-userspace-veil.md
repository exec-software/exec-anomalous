# ADR 0054: Prove a shared multi-caller userspace veil

- Status: Proposed; bounded proof qualified by 41 qualification checks and two restorations
- Date: 2026-10-04
- Related: ADR 0045, ADR 0050, ADR 0053

## Context

The qualified one-caller veil relocates Linux policy into userspace but its
sealed waiting authority can only be reused by the same caller. Antimatter's
runtime will need independently owned clients sharing supervised services.
Native task, copy, channel and lifecycle mechanisms remain the reference;
foreign numbers and errors remain policy inside the veil.

## Decision

Add a removable `shared_veil_test` proof with two independent Linux editor
producers, two independent native editor services and one shared native-entry
veil. Keep the qualified userspace artifacts unchanged. Each producer has its
own task generation, root, transition stack, CPU image and two channel bindings.
One raw request is outstanding at a time. This deliberately bounded serial
service does not claim concurrent request queues or asynchronous replies.

A move-only kernel service handle binds its original scheduler, an unrepeatable
service instance, exact veil generation and two exact caller generations.
Registration occurs before dispatch. Shared handoff requires live membership;
legacy one-caller handoff remains caller-pinned. Copied requests retain opaque
nonwrapping sequence values, with no task identity, memory root or channel owner
exposed. Mechanisms select the actual suspended caller's configured channels,
never a userspace-supplied identity. Completion keeps the existing preflight,
inert staging and infallible reply-commit boundary.

Caller retirement removes only its own membership and closes its own channels.
The surviving caller can continue through the same veil. The last caller's
retirement also retires the waiting veil. A veil fault discards its staged
effect, revokes the service and faults every still-dependent caller, including
a queued caller. Unrelated native services retain FIFO order and receive closed
routes. No Linux errno is invented by the kernel, and no restart is claimed.

The five retained tasks require 125 frames: 80 data frames, 45 table frames and
25 bound leases. Construct owners directly in retained BSS and inspect emitted
stack bounds. The previous three-task and native profiles remain independently
selectable; normal images exclude all shared-veil mechanisms. Only this new
profile uses linker section garbage collection to discard unreachable compiler
library functions. Threshold's existing 384 KiB staging bound is unchanged,
and post-link inspection still requires all twelve qualified embedded blobs.

## Acceptance

Run the native baseline followed by three five-task scenarios: both producers
complete; the first producer faults while the second completes; and the shared
veil faults after staging the first producer's surface. Compare both healthy
pairs' seven editor-state hashes and final pixels against the native reference.
Prove per-caller copy/channel isolation, exact reply correlation, membership
retirement and shared-fault revocation with host negative tests and target
observations. Require exact attributable transcripts and full owner reclamation.

Qualify both guest sizes under TCG/Intel and KVM/AMD, preserve the preceding
artifacts, inspect exact emitted kernel/userspace stack and privilege-entry
behavior, run repository-required regressions and restore development images.
Serialize all compiler/QEMU jobs under explicit memory limits and OOM monitoring.

## Limits

This proof uses a fixed two-member roster, one synchronous request and fixed
startup/controller grants. It does not implement arbitrary service discovery,
production multi-process Linux compatibility, full Antimatter/Electron, TLS,
threads, dynamic linking, filesystem services or native Reliquary packaging.
Ny remains the official language direction; existing qualified Rust is the
implementation substrate for this proof. HAG and Displacer are unchanged.

## Qualification receipt

All 41 qualification checks and two image restorations passed. The final repair
is limited to three shared-profile source files; 38 unaffected checks retain
the preceding receipts, while host checks and shared TCG/KVM proofs were rerun
on the final frozen set of 327 source/configuration files. TCG/Intel and KVM/AMD passed both guest sizes and
independent captures; all twelve preceding artifact blobs remain unchanged.
The final kernel matches the independently reviewed artifact exactly. No host
or cgroup OOM occurred. See the [Shared Veil Log](../../notebook/Development/2026-10-04%20Shared%20Veil%20Log.md)
for stack, lifecycle, memory, image hashes and retained failed-attempt evidence.
