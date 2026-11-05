---
title: "User-Space Display Presentation Log"
date: 2026-09-19
tags: [development, display, user-space, desktop, privacy, security]
status: accepted
domain: graphics
---

# User-Space Display Presentation Log

## Starting point

Before ADR 0024, Threshold transferred one validated 1024x768 packed-pixel
framebuffer to the kernel, and the kernel had one private canvas for boot
diagnostics and the bootstrap shell. ADR 0019 reserved display-update authority
and a display-patch payload class, but no versioned body or handler existed.
The pre-ADR0024 Rite task shape mapped only code, mailbox, and stack; the normal
image had no user-space Rite runtime.

The immediate goal is not to smuggle a desktop into the kernel. It is to prove
the smallest protected presentation mechanism on which a later user-space
semantic compositor can rely, while producing a visible artifact that Nathan
can review as an end user.

## Implemented experiment

ADR 0024 adds a removable `display_vessel_test` image. One fixed ring-3
client receives only display-update and task-exit rights. It submits exact,
versioned RGB8 rectangles through its existing one-page RW+NX Rite mailbox;
the kernel copies and validates each complete patch before an exclusive
presenter converts it to the boot framebuffer.

The client never receives the framebuffer address, pitch, channel masks, or a
user mapping of the aperture. Rejected, malformed, stale, and out-of-bounds
requests leave the framebuffer byte-for-byte unchanged. Valid requests
assemble one declared nonsecret desktop-shaped fixture and then retire the
task and all feature-owned state.

The QEMU acceptance harness retains the completed pixels after teardown,
captures a strict P6 screenshot, compares every pixel with an independent
public fixture oracle, and keeps the artifact under ignored `build/`. A
companion interactive command leaves the same proved frame visible in a QEMU
window for subjective layout and visual-direction review.

## Observed evidence

At implementation checkpoint `b6546cc`:

- the feature task mapped only one RX code page, one RW+NX mailbox, and one
  RW+NX stack, and held exactly `display_update | task_exit`;
- the task root's complete framebuffer aperture was explicitly walked and
  remained supervisor-only;
- 772 ring-3 calls produced three ordered rejection probes, 768 accepted
  full-width row commits, and one terminal exit;
- each denied, malformed, or out-of-bounds call preserved every byte of the
  canary-backed framebuffer aperture;
- exit retired the task without returning to it, cleared copied pixel storage,
  withdrew the channel, mappings, root, and leases, and restored all tracked
  accounting;
- the strict QEMU verifier accepted exactly 16 records, seven from boot and
  nine from the display proof; and
- an independent host oracle matched every one of the captured frame's
  786,432 RGB pixels.

The acceptance pass also completed `make check`, all established non-VirtualBox
QEMU smoke targets, `normal-exclusion`, warning-denied host and x86-64 Clippy,
an explicit all-feature freestanding build, and a strict final ELF link. The
focused suites passed 307 display-feature kernel tests and 46 `xtask` tests;
the shared ABI passed 22 unit and six compile-fail documentation tests,
`anomalous-session` passed 51 unit and 56 compile-fail documentation tests, and
`anomalous-session-router` passed 24 unit and 46 compile-fail documentation
tests.

The reproducible artifacts are `build/display_vessel.ppm` and
`build/qemu_display_vessel.log`. `make display-demo` builds and inspects the
same feature image, then leaves the completed public frame visible for end-user
review. VirtualBox was not run or required for this removable QEMU proof.

## Architectural boundary

Pixels remain a presentation projection, not authoritative desktop state. A
later user-space scene model must derive both accessible human rendering and a
machine-readable semantic projection from one revision. This slice adds no
focus, input, accessibility tree, action model, screenshots-as-authority,
telemetry, window protocol, or compositor claim.

## Acceptance status

**Accepted.** ADR 0024 accepts the bounded presentation mechanism and its
evidence at checkpoint `b6546cc`. It does not accept a compositor, window
server, production display manager, stable graphics ABI, or semantic desktop.

## Next experiment

Define one bounded canonical user-space desktop scene whose single committed
revision derives both an accessible render plan and a machine-readable
semantic projection. The next slice should prove revision ownership,
deterministic projection, focus/action identity, privacy-safe observation, and
human/machine equivalence before it claims window composition or input policy.
