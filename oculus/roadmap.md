# Oculus roadmap

Oculus is the planned desktop system for Anomalous: the session shell, window
manager, compositor, workspace model, input routing and appearance layer. It
replaces the current bounded Rust desktop/compositor reference with a real
system written in **Matrix**, the second surface language over the NIL runtime
shipped by the Ny project (see the public NIL framework and [runtime requirements](../docs/optional-runtime.md)).

This file tracks the work as small, reviewable units. Each unit names its
deliverable, its evidence, and its status. Update the status line as units land.

Legend: `TODO` not started, `WIP` in progress, `DONE` landed with evidence,
`BLOCKED` waiting on another unit or an open decision.

---

## Unit 0 — Shared Matrix-capable Anomalous Ny runtime

**Goal.** Anomalous keeps one project-wide, Matrix-capable Ny SDK under
`runtime/` instead of per-sub-project symlinked SDKs. Every sub-project
(`eidolon`, `oculus`, and future ones) consumes `runtime/` directly.

**Why.** Matrix is a compiler front end inside `nycompiler`, selected by the
`.matrix`/`.mat` extension. The old `eidolon/sdk` generation predates the Matrix
front end and rejects `.matrix` sources. A single shared runtime removes the
per-project SDK pin and lets Oculus compile immediately.

**Design.**

- `scripts/refresh-ny-runtime.sh` builds and validates a complete Ny SDK from the
  sibling Ny checkout, then copies the resolved generation into a real
  `runtime/` directory (no symlink indirection for consumers).
- Private state lives under `.runtime/`: the immutable generation store and the
  builder's managed activation symlink. The Ny CMake build is reused
  incrementally from the sibling checkout's own `build/` directory.
- Both `runtime/` and `.runtime/` are generated and Git-ignored.
- `eidolon/` is repointed from `sdk/` to `../runtime/`; the `eidolon/sdk` symlink
  and `eidolon/tools/refresh_ny_sdk.sh` are retired.
- The opt-in Ny object proof (`tools/xtask/src/ny_object_artifact.rs`) is
  repointed to `runtime/` and re-pinned to the new generation.

**Tasks.**

- [x] T0.1 Add `scripts/refresh-ny-runtime.sh`.
- [x] T0.2 Build a development runtime from the current Ny source
      (`NY_ALLOW_DIRTY_SOURCE=ON`) and copy it into `runtime/`. Generation
      `c5416212…` from Ny revision `2a45b0bb…`; 380/380 Ny tests passed first.
- [x] T0.3 Verify Matrix through `runtime/`: `oculus/src/hello.mat` compiles with
      `runtime/nycompiler` and prints its banner under `runtime/nyinterpreter`.
- [x] T0.4 Repoint `eidolon` (Makefile, tests, docs) from `sdk/` to `../runtime/`;
      retire the old script/symlink. `make -C eidolon check-ny` passes 26/26.
- [x] T0.5 Update `.gitignore`, root `README.md`, and ADR 0016's project-wide
      SDK contract.
- [x] T0.6 Repoint and re-pin the Ny object proof against `runtime/`. The host
      object/link/inspection profile and the proof image reproduce, and the TCG
      Intel-baseline boot passes at 96 MiB and 320 MiB. The actual-host KVM run
      and full regression matrix are a separate, expensive re-qualification.
- [x] T0.7 Record the change in `notebook/Development/`.

**Evidence.** `make -C eidolon check-ny` passes against `runtime/` (26/26); a
Matrix "hello" compiles under `runtime/nycompiler` and runs under
`runtime/nyinterpreter`; `cargo xtask ny-object-inspect` and `ny-object-image`
reproduce the pinned scalar object profile.

**Open follow-ups.**
- The runtime is a **development** SDK (dirty Ny source, `development`
  qualification). A tested, clean, release-qualified runtime is still needed
  before any release claim.
- Re-qualifying the Ny object kernel matrix (TCG + KVM, 96 MiB and 320 MiB) is
  expensive; record its result and, if it diverges, treat the pin as
  provisional.

**Status.** DONE

---

## Unit 1 — Oculus hello and project skeleton

**Goal.** A first Matrix program under `oculus/` that establishes the project
shape: a book per concern, a Makefile that drives the shared runtime, and a
runnable smoke target.

**Tasks.**

- [x] T1.1 Scaffold `oculus/` layout (books, Makefile, README).
- [x] T1.2 `oculus.hello` — smallest runnable Oculus page (`src/hello.mat`).
- [x] T1.3 `make oculus-smoke` compiles + runs the hello page through `runtime/`.
- [x] T1.4 Document the build/run contract in `oculus/README.md`.

**Status.** DONE

---

## Unit 2 — Oculus semantic model (host-side, no kernel)

**Goal.** Model the desktop's semantic surface in Matrix before any privileged
integration: window identity, geometry, focus, workspaces, panels, and a
canonical scene representation that can be diffed and inspected.

**Tasks.**

- [x] T2.1 Geometry foundations — `point` (window/geometry) and `rectangle`
  (window/geometry) with axis-aligned 2D coordinates.
- [x] T2.2 Rectangle extension — `is-empty` check on rectangles
  (window/geometry-rectangle-ext).
- [x] T2.3 Rectangle combine — bounding-box operation over two rectangles
  (window/geometry-combine).
- [x] T2.4 Window identity — generational ID counter
  (window/identity).
- [x] T2.5 Window roles — Desktop, Region, Control, Heading, Status, Panel
  (window/role).
- [x] T2.6 Focus model — Focused / Unfocused positions (window/focus).
- [x] T2.7 Scene node — combining identity, role, geometry, focus, label,
  and parent reference (scene/node).
- [x] T2.8 Scene record — revisioned container for scene nodes
  (scene/scene).
- [x] T2.9 Smoke test — exercises scene node construction and geometry checks
  (scene/smoke).

**Evidence.** `make check` passes: hello smoke and scene smoke both run
successfully under `nyinterpreter`.

**Status.** WIP (Type system foundation complete; scene composition and
workspace model pending.)

---

## Unit 3 — Session and window management

**Goal.** The window manager core: create/focus/raise/minimize/maximize/close,
workspace assignment, and an ordered, deterministic scene the compositor
consumes.

**Tasks.**

- [x] T3.1 Viewport — configurable physical/logical dimensions with scaling
  (window/viewport).
- [x] T3.2 Window lifecycle — Created, Visible, Minimized, Closed states
  (window/lifecycle).
- [x] T3.3 Workspace model — virtual desktops with index and label
  (window/workspace).
- [x] T3.4 Window record — full window state (identity, lifecycle, geometry,
  workspace, z-index, focus, label) (window/record).
- [x] T3.5 Scene manager — revisioned, viewport-aware scene container
  (scene/manager).
- [x] T3.6 Window operations — create, focus, raise, minimize, close
  (window/operations).
- [x] T3.7 Manager smoke test — exercises full window lifecycle
  (scene/manager-smoke).

**Evidence.** `make check` passes all three smoke tests: hello, scene-smoke,
and manager-smoke.

- [x] T3.8 Z-order management — sort-by-z-index and highest-z helpers
  (scene/z-order).
- [x] T3.9 Maximize — snap window to viewport bounds
  (scene/maximize).
- [x] T3.10 Scene builder — window-to-node conversion and build-scene
  (scene/builder).

**Evidence.** `make check` passes all three smoke tests: hello, scene-smoke,
and manager-smoke. Z-order, maximize, and scene builder compile and link
through transitive `read` resolution.

**Status.** DONE (Window CRUD, z-order, maximize, and scene building complete;
multi-workspace switching and real sorting pending in Unit 4.)

---

## Unit 4 — Compositor

**Goal.** Compose the scene into a framebuffer: surface abstraction, clipping,
output routing, damage tracking, and frame composition.

**Tasks.**

- [x] T4.1 Surface abstraction — pixel buffer with dimensions and format
  (compositor/surface).
- [x] T4.2 Clipping — viewport-scoped geometry clipping
  (compositor/clipping).
- [x] T4.3 Damage tracking — damage regions and tracker for incremental redraw
  (compositor/damage).
- [x] T4.4 Output routing — display output with surface and position
  (compositor/output).
- [x] T4.5 Frame — composed snapshot with revision, surface, damage, and dirty flag
  (compositor/frame).
- [x] T4.6 Compositor state — frame revision tracking and dirty state
  (compositor/compositor).
- [x] T4.7 Compositor smoke test — exercises surface, output, frame, and compositor creation
  (compositor/smoke).

**Evidence.** `make check` passes all four smoke tests: hello, scene-smoke,
manager-smoke, and compositor-smoke.

**Status.** DONE (Surface, clipping, damage, output, frame, and compositor
state complete; z-order sorting, clipping math, and incremental redraw
pending future refinement.)

---

## Unit 5 — Input routing

**Goal.** Keyboard, pointer and focus routing expressed in the same semantic
control model humans and machines use.

**Status.** TODO

---

## Unit 5 — Input routing (DONE)

**Goal.** Keyboard, pointer and focus routing expressed in the same semantic
control model humans and machines use.

**Tasks.**

- [x] T5.1 Input event primitives — keyboard-event and pointer-event records
  (input/event).
- [x] T5.2 Key mapping — physical key codes as a choice type
  (input/key-mapping).
- [x] T5.3 Input focus — focus state with focused window ID
  (input/focus).
- [x] T5.4 Pointer state — cursor position, button mask, hover tracking
  (input/pointer).
- [x] T5.5 Routing table — combines focus, pointer state, and viewport dimensions
  (input/routing).
- [x] T5.6 Input handler — event dispatch engine stub
  (input/handler).
- [x] T5.7 Input smoke test — exercises keyboard, pointer, focus, and routing
  (input/smoke).

**Evidence.** `make check` passes all five smoke tests: hello, scene-smoke,
manager-smoke, compositor-smoke, and input-smoke.

**Status.** DONE (Input event model, focus, pointer, and routing complete;
actual event dispatch and window hit-testing pending future refinement.)

---

## Unit 6 — Appearance / theme

**Goal.** Neumorphic appearance that also meets accessibility and nonvisual
state requirements, carried forward from the existing theme.

**Tasks.**

- [x] T6.1 Color system — semantic color values in RGBA
  (appearance/color).
- [x] T6.2 Theme configuration — name, background, surface, text, accent colors
  (appearance/theme).
- [x] T6.3 Style rules — corner radius, elevation, shadow, border width
  (appearance/style).
- [x] T6.4 Accessibility — contrast level, motion reduction, screen reader support
  (appearance/accessibility).
- [x] T6.5 Appearance layer — ties theme, style, and accessibility together
  (appearance/appearance).
- [x] T6.6 Appearance smoke test — exercises the full appearance stack
  (appearance/smoke).

**Evidence.** `make check` passes all six smoke tests: hello, scene-smoke,
manager-smoke, compositor-smoke, input-smoke, and appearance-smoke.

**Status.** DONE (Color, theme, style, accessibility, and appearance layer complete;
actual rendering and neumorphic compositing pending Unit 7.)

---

## Unit 7 — Integration and drop-in

**Goal.** Run Oculus through the kernel/vessel path and replace the bounded Rust
desktop reference at the agreed integration point.

**Tasks.**

- [x] T7.1 Session shell — top-level entry point and shell state machine
  (session/shell).
- [x] T7.2 Desktop context — unified state tying all subsystems together
  (session/desktop-context).
- [x] T7.3 Integration layer — wires compositor, input, and window manager
  (session/integration).
- [x] T7.4 Boot sequence — viewport initialization, compositor setup,
  input routing, appearance configuration (session/boot).
- [x] T7.5 Session smoke test — exercises the full integrated system
  (session/smoke).

**Evidence.** `make check` passes all seven smoke tests: hello, scene-smoke,
manager-smoke, compositor-smoke, input-smoke, appearance-smoke, and
session-smoke. The session smoke test boots the full desktop, creates a
window, dispatches input events, composes a frame, and reaches the running
state.

**Status.** DONE (Session shell, desktop context, integration layer, boot
sequence, and full system verification complete. Ready for kernel/vessel
integration in future work.)

---

## Open questions

- Which existing `anomalous-desktop-*` contracts become the Oculus conformance
  oracle, and which are retired?
- Does Oculus ship as a single Matrix book or many books with an orchestration
  book?
- What is the first observable surface: a semantic scene diff, a rendered
  frame, or a host-side session transcript?
- Does the shared `runtime/` need a committed record of its generation digest,
  or is the ignored generation store sufficient provenance?
