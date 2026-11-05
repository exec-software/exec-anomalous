# ADR 0034: Build shared native frame assets and canonical control states

- Status: Proposed
- Date: 2026-09-26

## Context

ADRs 0032 and 0033 establish the development workspace, canonical window state,
first theme components, and native pointer controls. The next increment needs
consistent frame geometry and interaction states that can grow into a polished
desktop without creating a second asset or state model beside the renderer.

The requested design reference is the local Antimatter project's Clay theme.
Its neutral charcoal surfaces, warm text, orange primary cues, and teal
secondary accents inform Anomalous Dark. Anomalous Light interprets the same
roles with warm light surfaces and darker orange and teal inks. The desktop's
future name remains undecided. This is visual direction, not a dependency on
Antimatter's runtime or a claim of production desktop readiness.

## Decision

### Keep authored controls and native pixels connected

Define original minimize, maximize, restore, and close controls as bounded
round-cap vector segments in `anomalous-desktop-theme`. Store one authoritative
24-unit recipe per control. The native compositor samples these compiled
recipes with bounded integer coverage, without allocation, unsafe code, an SVG
parser, filesystem reads, executable theme code, or external assets.

Export symbolic SVGs, theme-specific frame and control-state reference sheets,
and a small manifest from those same recipes and compiled theme definitions.
The exporter supports a deterministic check that rejects missing, extra, or
changed generated files. Checked-in reference artifacts are reviewable source
companions. They are not a runtime package, stable external theme format, or
new authority boundary; the manifest remains an internal experimental export.

SVG control geometry and native glyph geometry share their source. Browser SVG
rasterization, viewer fonts, and illustrative frame shadows are not the native
rasterizer. Frame sheets show design states and token use; they are not exact
framebuffer goldens or independent evidence of native presentation.

### Validate the complete frame appearance

Extend the compiled theme contract with an independently replaceable decoration
palette and bounded frame style. Resolve global theme, colors, metrics,
decoration colors, and frame geometry into one validated definition before
preparing a canonical revision. Reject incompatible color components or invalid
geometry without changing the published state.

Use neutral active and inactive titles, readable text, a distinct focus outline,
and original symbolic controls. Validate foreground/background pairs for normal,
hovered, pressed, and destructive states. Nonessential separators may be quiet;
focus, control glyphs, and task state must remain legible. A two-color focus
outline and explicit fallback inks preserve contrast when a custom or
high-contrast background inverts a normal role. Native raster size choices must
also preserve readable control cores rather than relying only on source colors.

The frame style bounds corner radii, border and separator widths, control
rounding, shadow spread, and shadow opacity. Static shadows blend into the
existing destination instead of replacing lower content with an opaque block.
Maximized and tiled windows use square outlines and omit floating shadows.
Depth remains optional; High Contrast retains its flat default. This increment
adds no animation, GPU effect, new font backend, output scaling, or asset loader.

Painting and window picking use the same rounded silhouette. Transparent
corners do not become rectangular input blockers. Frame controls retain their
shared bounded hit regions; a disabled maximize control in a tiled layout is
visibly disabled and cannot dispatch a maximize action.

### Publish pressed state through the canonical model

Pointer position and the bounded three-button state belong to the same
canonical workspace snapshot as the windows and appearance. Both local human
and machine clients use the same granted typed commands and observation path.
A physical pointer packet contributes position, button state, and any window
action to one bounded atomic batch, still limited to four commands.

The renderer derives hover and pressed appearance from the published snapshot
and shared hit geometry. It does not keep a private visual-only button latch.
Pointer loss clears published button state through an ordinary candidate and
presentation acknowledgement; a held or stale source cannot begin a new gesture
before the required release. Invalid button masks, denied commands, or failed
batch elements leave the published state unchanged.

These local semantics do not authenticate an external process or agent, grant
new kernel rights, or permit logging raw input. Existing presentation gating,
source sequencing, capture cancellation, and development-shell teardown remain
in force. Real applications, private surfaces, and cross-process semantic IPC
remain separate boundaries.

## Alternatives and reversibility

Independent hand-drawn SVGs and native bitmap controls would be easy to start
but could drift in geometry and state. Loading SVGs or theme code at runtime
would introduce parser, storage, provenance, and execution boundaries before
those services exist. Keeping pressed state only inside the renderer would
break the shared human/machine model.

Compiled recipes keep this increment bounded and removable. Preset tokens and
original geometry can evolve with reviewed generated diffs; a later external
package format requires its own design and validation. Accepted fixture ADRs,
ordinary boot behavior, and earlier native transport contracts are not amended.

## Validation

Require recipe bounds and coverage tests, deterministic export checks, complete
color-state validation, invalid-override rejection, and shared paint/pick
geometry tests. Exercise rounded clipping, overlapping windows, floating versus
maximized/tiled frames, disabled controls, pointer loss, and pressed-state
publication through the ordinary model boundary.

Verify actual native public frames separately with the workspace QEMU smoke.
Host/native agreement using the same compositor is cross-build and transport
evidence, not an independent full-rendering oracle. Retain independent directed
pixel and semantic assertions alongside it. Run the complete repository
validation matrix and normal-image exclusion before handing off the code;
record measured results in the notebook without turning this proposal into an
accepted production claim.
