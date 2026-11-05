---
title: "Native Workspace Development Log"
date: 2026-09-26
tags: [desktop, windows, workspace, compositor, ring3, input, privacy, qemu]
status: implemented-and-validated
domain: development
---

# Native workspace development

The user wants a production desktop foundation with windowing, composition,
and extensive customization for agents and humans, and a VM that can remain
open while development continues. Proposed
[ADR 0032](../../docs/adr/0032-start-a-native-window-workspace-and-compositor-service.md)
starts a separate native workspace path. It preserves the host recovery work
in ADR 0030, the bootstrap launcher in ADR 0031, and the exact accepted
ADR 0025–0029 fixtures.

## Portable policy and composition

`anomalous-desktop-workspace` is allocation-free, unsafe-free, and `no_std`.
It owns four window slots, two workspaces, and four local client endpoints.
All content is a fixed public selector: Welcome, System Inspector, or Activity.
There is no arbitrary string/content ingestion or private application surface.

The canonical model covers window creation/close, focus/raise, move/resize,
minimize/restore, workspace switches and membership, per-workspace floating or
tiled layout, and theme, density, depth, and reduced-motion preferences. IDs
include nonwrapping slot generations. Floating geometry survives horizontal
and vertical tiling; focus/raise does not reorder tiles. Hidden, minimized,
or closed windows cannot retain active focus, and failed geometry or capacity
changes leave committed state intact.

Human and machine endpoints use the same typed commands and local grant checks.
The origin is attribution only. `prepare` requires observation plus the
operation grant because its move-only ticket exposes the candidate view.
Foreign, stale, or revoked owners cannot publish by copying public IDs or
revision numbers. These branded Rust owners do not authenticate a native app,
remote process, principal, or external agent connection.

Construction returns an unpublished initial candidate. Only one successor can
be pending. Preparation retains the old committed view; beginning presentation
gates committed observations and new actions. The trusted native adapter must
acknowledge the complete matching frame before publication. Dropping a ticket
does not publish or reopen an uncertain view. Accessibility and machine facts
are projections of the same immutable canonical view.

`anomalous-desktop-compositor` consumes that view and composes one RGB8 row at a
time without allocations or unsafe code. It paints opaque windows in stacking
order with clipped chrome and text, a two-tone focus outline and text label,
three palettes, density-sensitive spacing, and optional depth. Reduced motion
is exposed in canonical state; the first compositor has no animation.

## Native integration contract

The opt-in `workspace_dev` image runs the separately linked
`anomalous-workspace-service` vessel. Window state, keyboard shortcuts, layout,
and composition remain in ring 3. The kernel supplies task-bound copied
physical input, bounded interrupt waiting, display staging, and termination.

The image has inspected RX code/read-only constants and private RW+NX data/BSS.
An eight-page RW+NX extent contains a mailbox page followed by private service
data; a separate sixteen-page RW+NX stack preserves service state. Guard gaps
separate the three mappings. The service receives input-read, display-update,
yield, and exit rights, without a framebuffer mapping.

A frame consists of 768 ascending full-width RGB rows at 1024 by 768. The kernel
holds a private 576-frame stage, copies a complete frame to its exclusively
owned framebuffer, verifies readback, and acknowledges the exact sequence.
The vessel then attests and publishes that candidate. This does not establish
tear-free scanout, page flipping, GPU fences, or native external-client IPC.

Saved user frames, task identity, and root ownership must be checked on timer
and keyboard interrupt return. Waiting must check readiness before sleeping;
physical input loss must reset adapter modifier state. A bounded watchdog
contains stalled user computation. The first failure policy returns to the
bootstrap shell only after returning to a safe kernel continuation and
reclaiming service resources. Fault, watchdog, and orderly-exit paths discard
workspace state. ADR 0030's recovery retains a surviving coordinator and is not
implicitly deployed by this service.

## Development loop

- `make workspace-image` builds and inspects `build/workspace-dev.img`.
- `make workspace-run` launches **Anomalous Workspace Development** with a
  private per-run disk and bounded private serial/stderr files, releasing the
  shared artifact lock before the VM starts.
- `make workspace-smoke` exercises typed keyboard interactions, complete frames,
  and contained fallback. Retained captures are limited to the public demo.
- `make workspace-virtualbox` selects the workspace image in the existing
  **Anomalous Dev [Threshold]** VM after it is powered off.

| Key | Action |
| --- | --- |
| Tab | Focus next visible window |
| T | Cycle floating, horizontal, and vertical layout |
| C / D | Cycle palette / toggle density |
| Arrows / Shift + arrows | Move / resize focused floating window |
| N / Q | Create demo / close focused window |
| M / R | Minimize focused window / restore minimized window |
| 1 / 2 / W | Switch workspace / move focused window to the other workspace |
| G / A | Toggle depth / reduced-motion preference |
| Esc | Withdraw service and enter bootstrap shell |
| F11 / F12 | Exercise watchdog termination / controlled fault |

Two additional reserved diagnostics exercise rejection paths: F10 submits a
malformed mailbox, and Shift+F12 faults with an invalid saved direction flag.
These are development probes, not application shortcuts.

The native shell fallback has no command to reconstruct this workspace. Start a
fresh workspace VM to return to the initial windows. QEMU releases capture with
Ctrl+Alt+G; closing its window stops that VM. Mouse interaction is not implemented.

## Validation checkpoint

The portable workspace model passed 13 unit tests and 8 ownership compile-fail
tests. The compositor passed 10 unit tests and warning-denied Clippy. The native
service keyboard adapter passed 5 tests; `anomalous-abi` passed 28 unit and 6
compile-fail documentation tests, and the workspace-feature kernel passed 364
host tests. The model tests cover grants and human/machine parity,
stale/foreign/revoked owners,
publication gating, window reuse, focus and workspace transitions, tiling round
trips, checked geometry and capacity, monotonic-counter exhaustion, and native
storage bounds. Compositor tests include independent known-pixel overlap,
clipping and focus checks, invalid-row/output failure atomicity, and contrast
checks for actual text pairs.

A fresh `workspace-image` build passed payload and linked-image inspection.
The vessel has six RX pages and eight initialized RW bytes, with BSS admitted
within the private writable budget. This is packaging evidence, not a guest
runtime result.

The fresh native `workspace-smoke` run passed. Real keyboard input produced
21 complete public frames across focus, layout, theme, density, move/resize,
minimize/restore, workspace switches, creation, workspace transfer, close,
depth, and motion-preference changes. Every captured 1024 by 768 RGB frame
matched the host cross-build reference exactly.

Five native termination scenarios also passed:

| Trigger | Verified withdrawal reason |
| --- | --- |
| F12 | `controlled_fault` |
| F11 | `watchdog` |
| F10 malformed mailbox | `user_fault` |
| Shift+F12 invalid saved direction flag | `user_fault` |
| Esc | `task_exit` |

Each withdrawal reported `resources_restored:true` through the strict lifecycle
validator, and the fallback bootstrap shell accepted `version` and returned a
new prompt. This demonstrates service withdrawal and a working fallback, not
restoration of the discarded windows.

The graphical `workspace-run` launcher opened **Anomalous Workspace
Development** and its private serial transcript reached the first complete
frame. Its mapped window was verified with the title **QEMU (Anomalous
Workspace Development)** and a 1024 by 794 host window extent. The QEMU session
used an independent disk throughout the full regression suite. During hands-on
testing it presented five frames, then an orderly Esc withdrawal reported
`reason:task_exit` and `frames_presented:5`. QEMU remains open at the bootstrap
prompt; the user's current session was preserved.

The stable workspace image and live private copy have SHA-256
`0f253dd2a5759e771af03b296b71e82f3ba438e5845cb99fe64b345013e506c2`.
The verified public initial and customized captures were exported as PNGs and
visually inspected. The final live/stable image hashes still match, and the PNG
exports match the final validated captures.

This increment's VirtualBox startup was attempted. Image conversion and VM
configuration succeeded, but startup returned `VERR_SVM_IN_USE` because KVM
owns the host AMD-V facility. The VM is powered off; no host hypervisor settings
were changed. This establishes no VirtualBox guest-runtime or framebuffer result.

The fresh full 20-target validation matrix passed in 939.2 seconds overall:

| Target | Elapsed seconds |
| --- | ---: |
| `check` | 310.0 |
| `smoke` | 1.5 |
| `fault-smoke` | 1.4 |
| `circle-smoke` | 35.9 |
| `control-smoke` | 33.7 |
| `rite-smoke` | 33.2 |
| `scheduler-smoke` | 32.5 |
| `supervision-smoke` | 32.6 |
| `shell-vessel-smoke` | 32.8 |
| `memory-smoke` | 34.0 |
| `interrupt-smoke` | 32.5 |
| `input-wait-smoke` | 33.7 |
| `shell-smoke` | 0.6 |
| `display-vessel-smoke` | 33.1 |
| `supervised-renderer-smoke` | 36.4 |
| `semantic-surface-transport-smoke` | 85.4 |
| `semantic-presentation-smoke` | 81.1 |
| `desktop-smoke` | 51.1 |
| `workspace-smoke` | 6.5 |
| `normal-exclusion` | 31.2 |

`make check` covered formatting, package and kernel host tests, all 79 xtask
tests, existing proof artifact inspections and combined-feature links, normal
exclusion, the native workspace image, warning-denied Clippy, and script syntax.
The final workspace smoke again verified all 21 complete frames and five
withdrawal paths, and final normal-image exclusion passed. These are fresh
workspace-change regressions, separate from the earlier ADR 0031 evidence.
ADR 0032 remains Proposed; implementation validation does not change its status.

A native capture comparison using the host build of the same compositor is
cross-build and transport evidence. It is not an independent full-renderer
oracle; the separate known-pixel and palette tests supply independent bounded
rendering checks. Lifecycle diagnostics must contain bounded static fields,
not raw keys or command bodies. Private input markers must be absent.

## Production work after this increment

1. Admit independent application surfaces through task-bound, revocable grants,
   with private-content policy and client supervision.
2. Define a versioned semantic IPC API for authenticated external agents and
   human interfaces over the same commands and observations.
3. Add pointer/text input, focus/capture epochs, keymaps, and a real user-space
   shell, then durable declarative profiles with preview and rollback.
4. Define service-loss recovery that preserves or explicitly reconstructs
   canonical state, keeping uncertain presentation unavailable until resolved.
5. Measure damage tracking, multiple outputs, DPI scaling, GPU composition,
   accessibility transport, performance budgets, and broader hardware support.

This is a bounded production foundation with public development clients. It is
not a production desktop, application platform, deployed assistive-technology
bridge, or stable ABI.
