---
title: "Workspace Launcher and Reliquary Naming"
date: 2026-10-04
tags: [desktop, launcher, userspace, reliquary, virtualbox, validation]
status: qualified
---

# Workspace launcher and Reliquary naming

## User direction

Native Antimatter running and rendering is current product Milestone 1. Smaller
applications can exercise the route; a named ELF personality follows the native
baseline, with Windows and macOS later. The active Antimatter checkout remains
Electron/React/CodeMirror with Node services and substantial ongoing edits; this
slice does not modify it or claim an Antimatter port.

The user selected `.re` for Reliquary and rejected native path-length ceilings.
Accepted ADR 0061 records the suffix and no fixed global path-length/depth limit,
with explicit finite resource accounting and structured failure. Existing native
proofs still load inspected payload segments. Serialized Reliquary encoding,
parsing, trust policy and general application admission remain open.

## Implemented desktop slice

The canonical launcher opens Welcome, System Inspector, Activity, Appearance or
Graphics Lab. It uses no window slot; it restores an existing matching app or
creates one within current capacity/geometry policy. Graphics launches with the
service's retained surface descriptor and preserves content on reopen. No
Antimatter, terminal or file-manager placeholders are shown.

The panel contains Applications, running windows, two workspaces and Appearance.
The separate theme/density/font/cursor/layout controls are removed. Appearance
retains explicit settings and existing keyboard shortcuts remain when the menu
is closed. Menu routing consumes unrelated keys and dismisses on Escape before
the usual shell return. Outside press/release cannot click through, and stale
or canceled menu gestures cannot launch an app. Human/machine commands retain
the existing grants and publication contract.

See [the launcher contract](../../docs/architecture/desktop-launcher.md).
Commits `9282088` (direction) and `323eeae` (launcher) were pushed independently.

## Focused and native evidence

The final focused pass passed 51 workspace, 55 compositor, 48 input-service and
221 tooling tests, plus ten workspace compile-fail documentation tests.
Initial failures were test defects: a Graphics expectation allocated a different
surface identity, and old frame hashes described the superseded panel. The
identity assertion now retains the actual submitted descriptor. The rendering
test now compares all RGB bytes with surface-level row skipping disabled across
24 palette/metric/scene cases, retaining primitive clipping and the intentional
header-shadow exclusion. No expected hashes were learned from the new renderer.

The native QEMU workspace run passed 21 keyboard frames, 462 pointer/Appearance
frames and 25 Graphics frames. It retains two-session cleanup, fault/watchdog/
malformed-context/mailbox containment and working shell fallback. The independent
input oracle now includes menu release/cancellation, navigation, capacity,
outside-Close dismissal, stale close/reopen gestures and F1/Up/Enter launching
Graphics with exact surface pixels. Complete light/dark menu captures were
visually reviewed. The runner's historical phrase “existing pointer frames”
counts 182 desktop/launcher frames in this revision; 207 Appearance and 73
release-lifecycle frames make up the remaining 462-frame total.

Evidence is retained in
an external validation directory (not distributed). The frozen source
contains 376 non-Markdown files. Documentation changes are separate from this
code/artifact freeze. Focused/native validation peaked at 722,599,936 bytes
(about 689 MiB), with at least 21,340,934,144 host bytes available and no OOM,
soft-limit or hard-limit events. Jobs are serialized with two Cargo workers,
one test thread, 4 GiB soft/6 GiB hard memory limits and a 512 MiB swap cap.

## Completed qualification

The emitted stack audit for workspace ELF
`cc0aa73973dc38fa1f60e7d1e3d0b96512ef95ae42001dfd4e7e404039896dcb`
gives a conservative 55,032-byte entry bound in the 65,536-byte user stack,
leaving 10,504 bytes. All 432 register calls/tails and 21 local jump tables are
accounted for; no dynamic RSP, red-zone, SIMD/MMX/x87 or stack-join imbalance was
found. This relies on valid Rust enum state and the fixed ASCII paragraph
invariant documented in `stack-audit/stack-review.txt`; it is not a hardware
high-water measurement or a corrupted-memory guarantee.

All 16 required repository checks passed, followed by native-relaunch and
ELF-relaunch smoke checks and both development-image restorations: 20 jobs in
the regression runner, plus the separately completed workspace smoke. The
final 376-file code/configuration freeze remains unchanged. The normal, native
replacement and ELF replacement images are byte-identical to the persistence
checkpoint; only the workspace image changed. Its SHA-256 is
`a9bd1589bdb910f92fea7cd1cf3d5e444e9c3719912e37c8986c505c707e5fa8`.

The regression scope peaked at 2,338,222,080 bytes (2.18 GiB), with at least
15,830,896,640 host bytes available (14.74 GiB). Host/scope OOM counters and
soft/hard-limit events stayed zero. Host swap was nearly full, so the available
memory guard and strict worker serialization remained enabled throughout.

An isolated VirtualBox 7.2.6 run booted the new workspace image with 512 MiB and
one CPU. Capture waited for the completed first frame. The eleven-record boot
and workspace lifecycle matched the QEMU reference, with only the explicitly
checked memory-map/accounting values permitted to vary. All 786,432 RGB pixels
matched QEMU exactly. Its private VBoxSVC and VM stayed in the bounded scope,
cleanup powered off the private VM, and the default VM registry was unchanged.
This is a VirtualBox boot/render comparison; interactive gestures were qualified
in QEMU. It does not change the previously documented VirtualBox native/ELF
floating-point capability rejection.

With the user-owned VM powered off, `make -j1 workspace-virtualbox` rebuilt its
VDI and started **Anomalous Dev [Threshold]** with 512 MiB and two CPUs. Readback
confirmed the new disk, running state and a complete first workspace frame.
This interactive VM remains available to the user; it is separate from the
private one-CPU qualification VM. The host still reported zero OOM kills after
startup.

The final evidence manifest covers 581 files and passed full checksum
verification. Its SHA-256 is
`db5e5713f493ee0fe43ec10044c3f3814649313319f0d2d455a8b64dadb34421`.

Durable guest storage, general `.re` loading, native editor integration and
broader runtimes remain later implementation work.
