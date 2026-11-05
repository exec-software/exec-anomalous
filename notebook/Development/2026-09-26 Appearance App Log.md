---
title: "Appearance App Log"
date: 2026-09-26
tags: [desktop, appearance, settings, compositor, pointer, keyboard, validation]
status: active
domain: development
---

# Built-in Appearance app

Proposed [ADR 0035](../../docs/adr/0035-add-a-built-in-appearance-app-with-shared-choice-state.md)
adds a discoverable settings app to the existing native development desktop.
It builds on the validated Clay frame increment while retaining the same public
content, four-window capacity, local authority, and presentation boundary.

## Implemented scope

- **P** or the header button opens the singleton Appearance window. It restores
  minimized content and moves an existing instance to the active workspace;
  otherwise it creates a centered 840×568 window bounded by the work area.
  The initial Welcome and System Inspector windows remain unchanged.
- Sixteen fixed choices cover Light/Dark/High Contrast, density, bitmap text
  scale, cursor shape, rounded/square frames, static depth, reduced motion, and
  reset. Settings and per-window keyboard selection are canonical state, shared
  with machine and accessibility views through the existing local endpoints.
- **F6**/**Shift+F6** selects the next/previous choice. **Enter**/**Space** applies
  it. **Tab** continues to traverse windows. Pointer selection uses the same
  bounded layout as painting; small windows show a maximize prompt instead of
  hidden interactive controls.
- Opening requires manage plus arrange authority. Selection requires arrange;
  activation requires arrange plus customize. All mutations retain revision
  checks, whole-batch rollback, and publication after complete presentation.
- Incompatible retained palette overrides disable prospective theme choices.
  The keyboard can inspect a disabled choice without applying it. Component
  choices preserve unrelated settings; reset restores the entire initial
  settings value. The read-only enabled predicate normalizes a copied snapshot,
  so geometry also disables reset when custom workarea changes would strand
  windows. It mutates no owner state and reserves no transaction.
- Corner choices use a narrow radius override layered after a complete frame
  profile. They leave explicitly authored profiles unchanged and allow later
  theme switches to resolve their own shadow and control defaults. A directed
  High Contrast → Rounded → Dark regression guards against accidentally
  freezing High Contrast's zero shadow opacity into the next theme.
- High Contrast retains the requested depth preference but uses flat frames.
  Reduced Motion remains a stored preference with no animation backend.
  Settings are session-local and disappear when the workspace owner exits.
- Invalid geometry/theme, exhausted capacity, and too-small layout are bounded
  native policy rejections. Keyboard rejection retains the frame; pointer
  rejection cancels capture and publishes position/buttons only through a fresh
  transaction. Unexpected failures still withdraw and restore the shell.

There is no separate app process, private or arbitrary application text,
external agent connection, filesystem profile, new font backend, or runtime
theme loader in this increment. The Clay frame assets and earlier validation
records remain unchanged historical evidence.

## Final validation

**Observed:** after the narrow corner-override correction, the theme suite
passed 18 tests and the workspace model passed 31 unit tests and 8 compile-fail
documentation tests. Warning-denied host all-target and
`x86_64-unknown-none` Clippy passed for both crates. The deterministic exporter
check verified all 22 frame reference assets unchanged. The final input adapter
suite passed 23 tests and the compositor suite passed 37 tests, including
shared control geometry, occlusion, fallback behavior, and focus visibility.

The model checks include singleton restoration/reuse, full-capacity refusal,
three viewport sizes, grant unions, stale/hidden/unrelated targets, all sixteen
choices and navigation wrap, equal human/machine/accessibility observations,
retained overrides, disabled theme/reset choices, reset, and atomic late-batch
failure without consuming a transaction.

The corner regression switches High Contrast → Rounded → Dark → Shadows On
and verifies Dark's normal shadow profile survives. It also switches Dark →
Square → High Contrast, checks the retained preference versus effective
flatness, verifies explicit full-profile preservation and narrow-override
clearing, rejects radius values above 12, and verifies reset clears the
corner preference.

**Observed:** native workspace smoke passed in 155.0 seconds, retaining 124
existing pointer frames and adding 213 Appearance exercise frames, for 337
pointer/Appearance frames plus 21 keyboard frames. It covered explicit mouse
and keyboard settings, singleton reuse/restoration, cross-workspace reuse,
capacity rejection, and two fresh desktop sessions. Controlled fault, watchdog,
malformed context/mailbox, and orderly exit restored resources and left working
shell fallback. Native host/compositor agreement is cross-build and transport
evidence, not an independent full-rendering oracle; directed semantic and pixel
tests remain separate component evidence.

All 20 validation targets passed. The first `make check` completed before the
corner-override correction and was repeated after that review fix; the table
uses the final passing check result of 225.6 seconds. Its **967.7-second
total sums the final passing target durations**, rather than measuring elapsed
wall time or including superseded checks. The native workspace build and smoke
already included the corrected corner semantics.

| Target | Result | Seconds |
| --- | --- | ---: |
| `make check` | Passed | 225.6 |
| `make smoke` | Passed | 0.4 |
| `make fault-smoke` | Passed | 0.3 |
| `make circle-smoke` | Passed | 31.2 |
| `make control-smoke` | Passed | 33.8 |
| `make rite-smoke` | Passed | 31.0 |
| `make scheduler-smoke` | Passed | 30.6 |
| `make supervision-smoke` | Passed | 31.8 |
| `make shell-vessel-smoke` | Passed | 31.1 |
| `make memory-smoke` | Passed | 33.1 |
| `make interrupt-smoke` | Passed | 31.9 |
| `make input-wait-smoke` | Passed | 31.8 |
| `make shell-smoke` | Passed | 0.6 |
| `make display-vessel-smoke` | Passed | 33.0 |
| `make supervised-renderer-smoke` | Passed | 34.1 |
| `make semantic-surface-transport-smoke` | Passed | 79.1 |
| `make semantic-presentation-smoke` | Passed | 76.6 |
| `make desktop-smoke` | Passed | 46.9 |
| `make workspace-smoke` | Passed | 155.0 |
| `make normal-exclusion` | Passed | 29.8 |

## Final native artifact and interactive session

The final inspected workspace image SHA-256 is
`4e7a0692841df27f005109fc906bace8021ccd25f75a7f4e28752ed3d7518b06`.
A private copy was launched as **Anomalous Workspace - Appearance (Final)**,
preserving existing interactive sessions. QEMU PID 1298215 uses
`build/desktop-runs/run-appearance-1298197/workspace-dev.img`. Its private image
hash matches the inspected final image. Native service readiness and the
complete 768-row first frame were verified; stderr was empty.
VirtualBox was not rerun for this increment; the earlier host KVM/AMD-V conflict
remains historical evidence, not a new runtime result.

## Remaining work

Durable profiles, arbitrary colors and theme packages, scalable fonts, true
application clients, private surfaces, authenticated external-agent IPC, text
input/IME, login/audio services, multiple outputs, GPU composition, and
state-preserving recovery remain separate boundaries. A native console needs
move-only private text owners, disclosure-aware views, and bounded private
input/output lifetimes. Private text must not enter the current public `Copy`
workspace snapshot.
