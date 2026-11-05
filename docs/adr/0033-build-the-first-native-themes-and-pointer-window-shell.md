# ADR 0033: Build the first native themes and pointer window shell

- Status: Proposed
- Date: 2026-09-26

## Context

ADR 0032 establishes a persistent native workspace owner and CPU compositor.
It provides a real keyboard testing loop, but its three embedded color palettes,
static keyboard guide, and terminal shell fallback do not yet make a convenient
desktop. The next increment should establish the first official light and dark
themes while making ordinary window interaction possible. The desktop's eventual
name is deliberately undecided.

The requested direction resembles a configurable desktop such as KDE Plasma:
colors, cursors, fonts, decorations, effects, login appearance, and sound roles
belong to a coherent theme system. These components do not all have runtime
services yet. Theme metadata must not imply an audio driver, login manager,
font loader, general application platform, or executable extension system.

## Decision

### Make themes data consumed by the real renderer

Add a portable allocation-free, unsafe-free `anomalous-desktop-theme` catalog.
The first official presets are Anomalous Light and Anomalous Dark, with a
high-contrast option retained. Theme definitions contain named semantic colors,
window/panel metrics, supported bitmap typography, cursor settings, and bounded
static effects. Component settings can be changed independently of a global
preset; validate the complete settings object before accepting a revision.

The current font backend is explicitly identified, with real supported sizes.
Do not relabel the same bitmap as multiple font families. Login-screen and sound
roles are declared extension points with unsupported runtime capability, not
selectable fake services. No external theme parsing, code execution, asset path
loading, or durable configuration is admitted in this increment. The catalog's
version describes this experimental contract, not a stable public ABI.

The workspace owns canonical settings. The renderer reads them from immutable
views and applies the tokens to actual colors, typography, cursor, decorations,
shadows, and panel controls. High contrast and reduced motion retain independent
policy meaning; the current static compositor adds no animation claim.

### Use the same window controls for people and machines

Add canonical pointer position and maximize/restore geometry, preserving the
previous floating rectangle. A bounded command batch validates every operation
and every required grant against one revision, then creates one successor.
Cursor movement, focus, and a drag/control action therefore publish together.
Any failed element rejects the complete batch without partial state changes.

The native adapter converts physical pointer observations into ordinary typed
commands through its granted endpoint. Observation bytes and window IDs convey
no authority. Pointer capture names a generational window and is canceled on
release, input loss, incompatible keyboard action, hidden/closed window, or
workspace/focus change. Only eligible floating windows can be dragged or resized.
Geometry clamps to the work area instead of pushing windows beyond reach.

Paint real minimize, maximize/restore, and close controls; a panel provides demo
launch, task activation/restoration, workspace selection, and supported theme
component controls. Painting and hit testing share exact geometry. Tests must
cover occlusion and ensure an opaque top window prevents interaction with lower
windows. Cursor composition happens after windows and shell chrome.

### Add a narrow native pointer source

The experimental workspace feature alone enables the legacy PS/2 auxiliary
mouse and IRQ12/cascade. Preserve the accepted normal and fixture keyboard
initialization, interrupt masks, and transcript profiles. Shared 8042 draining
must distinguish keyboard and auxiliary bytes so either source's interrupt does
not consume or misclassify the other source's data.

Use a bounded three-byte relative packet decoder, a loss-aware private queue,
nonwrapping source generations and sequences, and bounded device negotiation.
Extend the copied 48-byte physical-input body with distinct pointer and pointer-
loss variants; keyboard and keyboard-loss encodings remain unchanged. Reject
unsupported flags and reserved bytes. Delta/button bytes remain private input
and never enter lifecycle telemetry.

The live service multiplexes keyboard and pointer delivery without losing the
idle-wake condition. Loss ends any gesture before fresh source data can act.
Validate IRQ12's saved task/root/frame just as for IRQ1, acknowledge both PICs
before any withdrawal, and contain malformed user state without halting the
kernel. Disable the auxiliary controller port and mask IRQ12/cascade before the
shell fallback. Pointer-unavailable hardware must retain a usable keyboard path.

### Make the development shell round trip explicit

After complete native teardown, the bootstrap shell can accept `desktop` to
start a fresh workspace without restarting QEMU. Reuse one kernel continuation
in a loop; do not recurse and accumulate stacks. Each new native owner receives
fresh task/mapping state, a fresh native input adapter, and a new pointer
source generation; the kernel keyboard transport remains continuous. Drop the shell's framebuffer owner before the
next presenter starts. The ordinary shell still has its five original commands;
the return command is available only in a development session.

This is fresh construction after withdrawal, not preservation of the old
windows or deployment of ADR 0030 service-loss recovery. Session theme/window
changes remain volatile. Existing bootstrap-launcher behavior remains covered
by its regression tests.

## Validation

Require independent theme/contrast, settings atomicity, batch/grant, pointer
packet/loss, gesture cancellation, maximize/restore, and occluded-hit tests.
Exercise the real native pointer path with QEMU monitor input, verify complete
public frames against the host build, and check semantic geometry independently
of the pixel comparison. Exercise light/dark and component controls, window
buttons, drag/resize, workspace/task switching, and repeated shell round trips.
Preserve the strict diagnostic allowlist and remove rejected capture artifacts.

Run the full repository validation matrix and the existing desktop/workspace
regressions. Check normal-image exclusion and combined-feature precedence.
Attempt VirtualBox when the host supports it. Record measured results in the
notebook separately from this proposed architectural contract.

## Remaining boundaries

General application surfaces, authenticated external-agent IPC, private content,
font/asset loading, login/session management, audio playback, durable profiles,
text input/IME, touch, GPU composition, output scaling, and service-state recovery
remain separate work. This increment makes the native development desktop easier
to use and establishes its theme vocabulary; it does not claim a complete desktop
environment or production readiness.
