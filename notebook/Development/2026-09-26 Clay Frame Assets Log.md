---
title: "Clay Frame Assets Log"
date: 2026-09-26
tags: [desktop, theme, compositor, assets, pointer, validation]
status: active
domain: development
---

# Clay-inspired native frame assets

Proposed [ADR 0034](../../docs/adr/0034-build-shared-native-frame-assets-and-canonical-control-states.md)
extends the existing native workspace with original compiled vector controls,
reviewable frame assets, and canonical pressed-state rendering. The official
names remain Anomalous Light and Anomalous Dark; Light remains the initial
preset. The desktop's own name is still undecided.

## Design reference and provenance

The local Antimatter checkout's `vendor/displacer/tokens/index.js` selects
**Clay**, rather than its older palette named `dark`. Its Displacer manifest
recorded the historical version 1.0.1 design reference. That source revision
is not a prerequisite for building the original Anomalous recipes.
`renderer/src/plugins/chrome.tsx`, `renderer/src/plugins/ToolWindowHost.tsx`,
and `renderer/src/about.css` supplied concrete local frame references. No
Antimatter glyph, font, or frame asset was copied.

Anomalous Dark adopts Clay's `#141414` background, `#1a1a1a` surface,
`#ece9e5` warm text, `#f2661a` orange primary, and `#33b7a6` teal accent.
Titles stay neutral rather than becoming large saturated bands. Light is an
Anomalous interpretation: `#faf7f2` warm surfaces, `#292620` text, `#a63b08`
burnt-orange primary, and `#11695f` teal retain readable contrast. High Contrast
remains available. The runtime still uses the existing embedded bitmap font.

## Implemented scope

- Original minimize/maximize/restore/close recipes live in
  `crates/anomalous-desktop-theme/src/assets.rs`. Bounded integer sampling and
  SVG export consume the same round-cap segment geometry.
- [Frame reference assets](../../assets/desktop/frames/README.md) include
  symbolic controls, Light/Dark/High Contrast control-state and frame sheets,
  a contact sheet, and an internal experimental manifest. The exporter's
  `--check` mode rejects missing, extra, or changed generated files.
- Independent decoration palettes and frame-style overrides join the existing
  semantic colors and chrome metrics. Validation rejects unreadable control
  states and unsafe geometry before publication. Normal, hovered, pressed,
  destructive, inactive, and disabled states have explicit treatment.
- Rounded native frames, bounded destination-blended shadows, shared silhouette
  picking, and square maximized/tiled frames use the actual theme definition.
  High Contrast and disabled controls retain visible state without relying on
  shadow or color alone.
- Pointer button state joins the canonical snapshot. Physical packets publish
  position, button state, and action together through the existing four-command
  maximum. Human and machine clients use the same operation grants and atomic
  revision boundary. Pointer loss clears button state through that boundary.

The SVG frame sheets are design references, not native pixel captures. They use
viewer fonts and reference shadow rendering; native composition owns its bitmap
text, coverage, shadow algorithm, and final pixels. There is no runtime SVG or
font loader, external theme package, new application client, or stable external
manifest format in this increment.

## Validation

Observed host checks passed 16 theme/asset tests and warning-denied host and
freestanding theme Clippy. Deterministic exporter checks passed. The final
compositor suite passed 31 tests, with warning-denied host Clippy, after the last
two directed fixes: clipping resize marks to both their bounded rectangle and
the rounded silhouette, and painting the focus inner keyline after other window
content so a thick custom border cannot erase it.

Final `make workspace-smoke` passed 21 exact keyboard frames and 124 pointer
frames, including controls and theme changes across two native sessions. The
controlled fault, watchdog, malformed saved context, malformed mailbox, and
orderly-exit paths all restored resources and left working shell input. The
workspace smoke completed in 42.5 seconds. Comparing native frames with the
host build of the same compositor proves cross-build and presentation agreement,
not an independent full-rendering oracle; directed geometry and pixel tests
supply separate component evidence.

All 20 validation targets passed in **867.0 seconds** total. `make check`
completed in 235.9 seconds; final normal-image exclusion took 30.1 seconds.
These are fresh results for the frame increment. The earlier full pass in the
[Native Themes and Pointer Log](2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md)
remains historical evidence for ADR 0033.

| Target | Result | Seconds |
| --- | --- | ---: |
| `make check` | Passed | 235.9 |
| `make smoke` | Passed | 0.3 |
| `make fault-smoke` | Passed | 0.3 |
| `make circle-smoke` | Passed | 32.4 |
| `make control-smoke` | Passed | 31.8 |
| `make rite-smoke` | Passed | 30.9 |
| `make scheduler-smoke` | Passed | 31.2 |
| `make supervision-smoke` | Passed | 31.3 |
| `make shell-vessel-smoke` | Passed | 30.8 |
| `make memory-smoke` | Passed | 31.1 |
| `make interrupt-smoke` | Passed | 33.0 |
| `make input-wait-smoke` | Passed | 30.5 |
| `make shell-smoke` | Passed | 0.5 |
| `make display-vessel-smoke` | Passed | 31.0 |
| `make supervised-renderer-smoke` | Passed | 33.6 |
| `make semantic-surface-transport-smoke` | Passed | 76.1 |
| `make semantic-presentation-smoke` | Passed | 84.6 |
| `make desktop-smoke` | Passed | 49.1 |
| `make workspace-smoke` | Passed | 42.5 |
| `make normal-exclusion` | Passed | 30.1 |

Review also corrected state/contrast edges in custom palettes and High Contrast:
disabled title controls and task underlines choose contrast-safe fallback inks,
native control sizes retain an opaque readable core, and blended shadows still
consume the existing shadow-offset override.

## Final native artifact and interactive session

The final inspected workspace image SHA-256 is
`ffad76f521e1aed968aa0b3a9a63baf3b0bc3aec1450e70587a1621ad72e6159`.
A fresh graphical QEMU opened as **Anomalous Workspace - Clay Frames
(Validated)**. Its native service readiness and complete 768-row first frame
were verified, with an empty stderr log. QEMU PID 886230 uses the private disk
`build/desktop-runs/run-clay-886212/workspace-dev.img`, preserving existing
interactive sessions. Final native PNG exports were visually inspected and
saved with the task's output artifacts.

VirtualBox was not rerun for these frame-only changes. The earlier
`VERR_SVM_IN_USE` startup failure remains a historical host limitation; this
increment claims no fresh VirtualBox runtime evidence.

## Remaining work

This is a production-oriented asset foundation inside the development desktop,
not a production environment. External asset/font loading, persistent profiles,
login and sound services, independently supervised applications, private
surfaces, authenticated external agents, text/IME input, multiple outputs, GPU
composition, and state-preserving service recovery remain separate work.
