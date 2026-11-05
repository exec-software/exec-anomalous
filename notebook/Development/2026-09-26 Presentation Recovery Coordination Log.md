---
title: "Presentation Recovery Coordination Log"
date: 2026-09-26
tags: [development, desktop, presentation, recovery, privacy, host-proof]
status: implemented
domain: semantic-control
---

# Presentation Recovery Coordination Log

## Starting point and scope

Resumed from clean `main` at `93799a2`, the acceptance of ADR 0029. Its native
proof tears down the source, renderer and supervisor before framebuffer
presentation; semantic publication then occurs in the retained host service.

The first new slice is proposed ADR 0030's host-only recovery coordinator over
the existing surface service. It retains the real private transaction, gates
semantic reads before external mutation, and separates complete-copy
attestation, semantic publication and final acknowledgement. A single fresh
presenter may redraw the exact retained successor after interruption.

This is recovery from presenter failure or lost acknowledgement **while the
coordinator survives**. It is not recovery after loss of the canonical service
process and is not a deployed native presentation protocol.

## Implementation

`anomalous-desktop-recovery` owns the ADR 0027 service and controls privately.
The renderer's exact lease is used with the ADR 0028 adapter to encode the
retained redacted body before completion. Public scalar correlation cannot
recreate the presenter, recovery-control or ticket owners.

One controller-authorized replacement invalidates the prior endpoint and
ticket. A complete redraw is required even after semantic publication, so a
lost final acknowledgement cannot reopen reads over uncertain pixels. A
retained publication outcome prevents a second semantic sequence increment.
Retirement remains possible without asserting that framebuffer contents have
been cleared.

The new package is included in the workspace and `make check`. The README now
also reflects the already-accepted native transport/presentation proofs and
lists their existing smoke commands.

## Validation

Focused validation passed 5 unit tests, 12 independent integration tests and
37 privacy/ownership compile-fail tests. These cover injected lower-layer
owner-return failures, counter exhaustion, six presentation interruption cuts,
the seven-phase rejected-operation matrix, all 64 valid and 32 invalid scene
states, foreign equal-public-ID owners, dropped tickets, later transactions,
and retirement. The privacy test compares both complete encoded bodies and
actual ordinal-61 raster output for unequal protected source values and lengths.

The tiny framebuffer interruption tests are software-contract simulations;
they do not claim native process faults or hardware presentation. The real
raster comparison separately checks redaction through the existing renderer.

Formatting, warning-denied focused Clippy and a locked freestanding
`x86_64-unknown-none` library check passed. Independent implementation and
contract reviews reported no substantive findings. Kernel and native renderer
dependency inspection confirms that neither depends on the new coordinator.

The full validation run passed:

- `make check`, including all configured host suites, native artifact
  inspections, combined-feature linking, normal-image exclusion, script
  syntax and warning-denied host/freestanding Clippy. The recovery package's
  host tests and freestanding Clippy are now part of this target.
- The serial QEMU matrix: `smoke`, `fault-smoke`, `circle-smoke`,
  `control-smoke`, `rite-smoke`, `scheduler-smoke`, `supervision-smoke`,
  `shell-vessel-smoke`, both `memory-smoke` sizes, `interrupt-smoke`,
  `input-wait-smoke`, `shell-smoke`, `display-vessel-smoke`,
  `supervised-renderer-smoke`, `semantic-surface-transport-smoke` and
  `semantic-presentation-smoke`.
- A final `make normal-exclusion` after the QEMU matrix and `git diff --check`.

QEMU results exercise the existing native proofs; they do not execute the new
host coordinator. No code fixes were needed during the full validation run.
VirtualBox was not run: this slice changes no boot or native-runtime code.
ADR 0030 remains Proposed and records the host contract and native follow-on
separately; these results do not accept a deployed native service.

## Constraints for the native follow-on

- Scene and surface private brands use writable `AtomicU64` storage. Current
  vessel linker scripts reject writable image sections. A native semantic
  owner needs a private RW+NX data mapping or a separately reviewed allocator
  design; writable state must not be placed in its RX image.
- The current circle mapping capacity is three and supervision capacity is
  one. Retaining a semantic service alongside supervised rendering needs a
  deliberate bounded extension or a different lifetime arrangement.
- The renderer vessel hardcodes revision 1/2, generation 1 and attempt 1;
  its transport rasterizer validates the exact ordinal-61 profile. A native
  initial-frame or retry proof must define and verify its fresh launch binding.
- A full native protocol must authenticate current task/channel/endpoint
  state and retain outcomes across failed reply copies. Wire fields alone
  must not supply publication authority.
- The present framebuffer baseline is a canary, not a rendering of revision
  1. Rollback needs an independently verified old frame and surviving canonical
  checkpoint. The host slice proves forward redraw under explicit
  unavailability instead.
- Protected canonical content will live in a native semantic service's private
  memory. Its privacy boundary differs from the redacted renderer, transport,
  framebuffer evidence and diagnostics.

The native-service milestone remains open. No Threshold, kernel, Rite ABI,
native vessel or normal-image runtime changes are included in this slice.
