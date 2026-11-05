---
title: "Native Themes and Pointer Log"
date: 2026-09-26
tags: [desktop, theme, compositor, pointer, input, ring3, validation]
status: active
domain: development
---

# Native themes and pointer window shell

Proposed [ADR 0033](../../docs/adr/0033-build-the-first-native-themes-and-pointer-window-shell.md)
extends the ADR 0032 native service. The desktop name is still undecided;
Anomalous Light and Anomalous Dark are the first official theme names.

## Implemented scope

- The new allocation-free `anomalous-desktop-theme` crate defines versioned
  compiled theme data, semantic colors, shared chrome metrics, supported bitmap
  typography, two cursors, and static depth. High Contrast remains available.
  Independent overrides are validated for contrast and geometry before a new
  workspace revision can be prepared. Login and sound roles explicitly report
  unavailable; external assets and durable configuration do not exist yet.
- The model owns canonical pointer position, maximize/restore state, and bounded
  atomic batches of up to four commands. Every operation's grants are required;
  any late failure discards the complete private candidate. At most one create
  occurs in a batch, preserving the single created-window publication result.
- The compositor draws real window controls and a task panel. Paint and pointer
  picking share rectangles, including top-window occlusion and stable task
  ordering. Cursor composition follows windows and panel chrome. Font size,
  density, theme, cursor style, layout, tasks, and workspace controls are live.
- The user-space pointer adapter derives gestures from the copied physical
  stream. Left press focuses or invokes controls; titlebar and lower-right grip
  drags clamp to the work area. Capture retains the exact window incarnation and
  expected revision. Release, keyboard actions, loss, or another revision cancel
  it. Loss also requires an all-buttons-released observation before a new click.
- The opt-in BSP PS/2 AUX source owns a bounded three-byte packet decoder,
  64-event queue, source generations, explicit loss/rearm, IRQ12 and cascade.
  Shared 8042 draining keeps keyboard and mouse bytes distinct; neither source's
  raw data is logged. The copied 48-byte input body gains strict pointer and
  pointer-loss variants while retaining the existing keyboard encodings.
- Escape returns to the bootstrap shell after complete resource teardown.
  `desktop` starts a fresh native owner in a loop rather than adding recursion.
  Pointer generations advance across entries. Windows and appearance reset;
  this does not deploy surviving-coordinator recovery or persistent settings.

## Review observations

Independent review found and corrected two backend edges before native testing:
shared input draining needed the existing bounded empty-keyboard-IRQ containment
and equivalent pointer containment; mouse setup also had to avoid reviving a
first port whose keyboard configuration had failed. Loss handling requires
sequence zero in the new pointer generation, and held buttons cannot restart a
gesture without release.

## Validation

The focused native QEMU smoke passed 21 exact keyboard frames and 124 exact
pointer frames across two native sessions. It exercised focus, titlebar dragging,
corner resizing, maximize/restore, minimize/task restoration, theme/density/font/
cursor controls, all three layouts, workspace changes, demo create/close, and
Esc → shell `version` → `desktop` → a new generation → Esc → `version`.
Five termination paths still restore resources and leave working shell input:
controlled fault, watchdog, malformed mailbox, malformed saved flags, and orderly
exit. Pointer source generation advanced from one to two on the fresh entry.

Host checks passed: theme 7 tests; workspace 19 unit plus 8 compile-fail checks;
compositor 21; service adapters 15; ABI 29 plus 6 compile-fail checks; workspace
kernel 374; all-feature kernel 381; xtask 80. `make check` passed in 251.8 seconds,
including formatting, artifact inspection, warning-denied host/freestanding
Clippy, precedence links, and normal-image exclusion. The complete 20-target matrix passed in 910.5 seconds total.
The final repeated workspace smoke passed the same 21 keyboard and 124 pointer
frames, both native sessions, and every termination case after the final UI fixes.

| Target | Result | Seconds |
| --- | --- | ---: |
| `make check` | Passed | 251.8 |
| `make smoke` | Passed | 1.3 |
| `make fault-smoke` | Passed | 1.2 |
| `make circle-smoke` | Passed | 33.2 |
| `make control-smoke` | Passed | 36.9 |
| `make rite-smoke` | Passed | 33.9 |
| `make scheduler-smoke` | Passed | 34.3 |
| `make supervision-smoke` | Passed | 33.7 |
| `make shell-vessel-smoke` | Passed | 33.7 |
| `make memory-smoke` | Passed | 33.4 |
| `make interrupt-smoke` | Passed | 33.3 |
| `make input-wait-smoke` | Passed | 36.4 |
| `make shell-smoke` | Passed | 0.6 |
| `make display-vessel-smoke` | Passed | 33.6 |
| `make supervised-renderer-smoke` | Passed | 35.0 |
| `make semantic-surface-transport-smoke` | Passed | 78.8 |
| `make semantic-presentation-smoke` | Passed | 77.6 |
| `make desktop-smoke` | Passed | 52.3 |
| `make workspace-smoke` | Passed | 24.7 |
| `make normal-exclusion` | Passed | 44.8 |

Comparing the native frame to the host build of the same compositor proves
cross-build and presentation agreement, not an independent full rendering oracle.

## Interactive VM and host limitation

The frozen inspected native vessel has 43,112 bytes of RX payload (11 pages) and
8 bytes of initialized private data. Its development disk SHA-256 is
`defbdcaf18d38973be463dd10837486de6991b570966f3bd969820798ece0ba1`.
A fresh graphical QEMU session opened as **Anomalous Workspace - Light and Dark**
while regressions continued. Initial frame presentation and pointer availability
were confirmed. The live VM uses a hash-checked private disk copy with owner-only,
64 KiB-bounded serial and diagnostic logs. The prior interactive VM was preserved.

VirtualBox conversion succeeded, but startup again failed with `VERR_SVM_IN_USE`
because KVM holds AMD-V on this host. No host virtualization settings were changed;
The dedicated VM remained stopped in the aborted state; VirtualBox has no
fresh native-theme runtime evidence.

Native injected pointer-loss recovery is not part of this smoke. Packet loss,
rearm bookkeeping, gesture cancellation, all-buttons release, stale revisions,
and wrong-generation/sequence handling have directed host tests. The full native
smoke verifies real ordinary mouse delivery, not every device fault.

## Remaining work

Real application processes and private surfaces, authenticated external agent
IPC, durable profiles, external fonts/cursors/theme assets, login/session
management, audio playback, text/IME/touch input, GPU composition and multiple
outputs, damage tracking, and state preservation after owner loss remain open.
