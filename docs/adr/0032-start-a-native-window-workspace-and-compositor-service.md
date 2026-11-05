# ADR 0032: Start a native window, workspace, and compositor service

- Status: Proposed
- Date: 2026-09-26

## Context

The interactive development image in ADR 0031 returns from a one-shot native
renderer to a kernel-hosted launcher and shell. It supplies an initial testing
loop, but cannot become the desktop policy owner without moving window,
configuration, input, and composition policy into ring 0. The production
architecture instead places these responsibilities in supervised user space.

ADRs 0025–0029 deliberately define exact fixture profiles. Their eight-node
scene, fixed geometry, copied 528-byte body, renderer binding, and terminal
lifecycle do not define a general window protocol. Extending them in place
would invalidate the meaning of their accepted evidence. ADR 0023's one-way
focus transfer also retires the previous session; live desktop windows need
reversible focus without destroying application state.

This increment starts a separate portable workspace core and software
compositor, exercised in one persistent native desktop vessel. It is a
production foundation with explicitly bounded development clients, not a
production-ready desktop or a general application runtime.

## Decision

### Own desktop policy in one native service

Add the allocation-free, unsafe-free `no_std` crates
`anomalous-desktop-workspace` and `anomalous-desktop-compositor`, plus the
separately linked `anomalous-workspace-service` vessel. The opt-in
`workspace_dev` kernel feature supplies only task-bound copied physical input,
strict bounded display rows, interrupt waiting, and contained termination.
Window, focus, layout, theme, and shortcut policy reside in user space.

One service initially owns both canonical state and software composition.
There are no arbitrary application processes, untrusted application buffers,
external agent connection, executable plugins, or private application content
in this slice. All demo content is a fixed, explicitly public selector. Do not
label these panels as a working terminal or a connected agent.

The existing scene, transaction, recovery, transport, and renderer proofs remain
separate. Ordinary boot remains separate. Combined feature builds retain the
accepted terminal proof precedence; the workspace runtime has a dedicated
image and launch command.

### Establish a canonical bounded workspace

The portable model supports four window slots, two workspaces, and four local
client endpoints. Window selectors include nonwrapping generations so closing
and reusing a slot cannot turn a stale selector into a new window. A complete
revision contains window existence, workspace membership, floating geometry,
visibility, stacking order, focus, layout, and configuration.

Floating placement and horizontal/vertical tiling are separate policies over
the same windows. Preserve floating rectangles while tiled. Tile order is
stable under focus/raise changes. Closing, minimizing, or moving the focused
window chooses a deterministic visible successor; hidden windows cannot retain
active focus. Geometry and resource exhaustion reject without partial effects.

Expose typed create, close, focus, raise, move, resize, minimize, restore,
workspace, layout, and configuration operations. Bind a local client only
through the privately branded controller, then check its granted rights and
freshness for each operation. Preparing an update requires both the operation
grant and observation authority, because the resulting private ticket exposes
the complete candidate view. Blind mutation is not part of this local API.
The human/machine origin tag is attribution, never authority. An agent request
supplies its explicit window target; it does
not inherit human focus. The keyboard adapter chooses an explicit target and
calls those same operations. These local owners do not authenticate a remote
process, principal, or network connection.

Human rendering, accessibility facts, and machine observations derive from the
same immutable canonical view. Public IDs and view bytes describe objects;
they neither mint local controls nor grant native device access.

### Publish complete state and complete frames

Construction starts with an unpublished candidate rather than asserting an
unverified initial screen. Permit only one pending candidate. Preparation
validates the whole successor; readers may still observe the committed
revision before presentation begins. Beginning presentation gates committed
observation and action dispatch until the complete corresponding frame is
acknowledged and the canonical successor publishes. Retain the pending owner
on rejected progression. Revoked or foreign owners cannot progress a pending
transaction merely by repeating public revision numbers.

The compositor reads an immutable candidate and composes complete RGB8 rows in
back-to-front order. Begin with opaque windows, clipped drawing, bounded
chrome, clear focus outlines, and a readable bitmap font. It never receives a
framebuffer pointer and never publishes semantic state.

The kernel copies exactly one full-width row per accepted request into one
private full-frame stage. Require ascending rows and an exact frame sequence.
No partial stage becomes visible. On the final row, copy the complete frame,
verify readback, acknowledge the exact sequence, and clear/reuse the stage.
The service publishes only after that acknowledgement. CPU writes and
readback do not establish tear-free scanout, page flipping, or GPU completion.

The initial native failure policy is explicit withdrawal of the service and
return to the bootstrap shell, with full task/mapping/stage reclamation. It
does not promise to preserve windows through service loss. ADR 0030's surviving
coordinator recovery policy remains separate; integrating equivalent recovery
with this general model needs its own evidence.

### Keep writable state private and execution bounded

The vessel has two inspected ELF load segments: RX instructions/read-only
constants and RW+NX initialized data/BSS. The writable segment sits after a
separate mailbox page within one exclusively task-owned RW+NX mapping. The
kernel accesses only that mailbox page through the fixed copied rites. This
retains the existing three-mapping capacity: executable image, private
mailbox/data extent, and private RW+NX stack. Guard gaps separate the regions.
No region is writable and executable; no raw framebuffer enters user space.

The service stays alive across input, waiting, and repeated presentations.
Admit its timer and keyboard interrupts only after exact saved user-frame,
root, and task validation. A bounded timer watchdog contains unresponsive user
execution. A yield checks readiness before sleeping for an interrupt; an input
loss clears adapter modifier state before fresh source delivery. Contained
faults return to a saved kernel continuation before any active stack or mapping
is destroyed. Existing terminal fault handling remains unchanged elsewhere.

### Make customization declarative and accessible

The first runtime settings select validated light, dark, or high-contrast
palettes; compact or comfortable density; floating or tiled layout; and bounded
depth/motion preferences. No animation is required for this slice. Focus and
state use text and shape in addition to color. Palette tests cover the actual
text pairs used by the renderer; this is scoped validation, not accessibility
certification or a deployed assistive-technology bridge.

Human controls and machine commands apply settings through the same revisioned
transaction. Invalid configuration cannot partially alter layout or policy.
Keep diagnostic/recovery shortcuts reserved. Later customization should add
versioned declarative profiles, per-workspace overrides, preview/rollback,
keymap policy, and capability-scoped extensions. Executable plugins and durable
profiles require separate process, storage, and lifecycle boundaries.

### Preserve an effective development loop

Provide `workspace-image`, `workspace-run`, `workspace-smoke`, and a
`workspace-virtualbox` development launch target. Build and
inspect the RX/data payloads before packaging `build/workspace-dev.img`.
Graphical launch copies that disk into its own private run directory and
releases the shared build lock before QEMU starts. Rebuilds and regression tests
must remain possible while a user tests the live workspace.

Native serial diagnostics contain only bounded lifecycle, resource, and frame
sequence fields. Do not log physical keys or command bodies. Smoke captures
are permitted only for the closed public demo profile, after strict lifecycle
checks and full comparison with the expected host-rendered view. That comparison
is cross-build/transport evidence using the same compositor; independent
clipping, overlap, focus, and palette unit tests supply separate rendering
checks. It is not an independent full-renderer oracle.

## Validation and rejection criteria

Require meaningful tests for grants, foreign/stale endpoints and tickets,
window slot reuse, focus/visibility, workspace transitions, tiling round trips,
checked geometry, rejected configuration, strict backpressure, and read/action
gating during presentation. Exercise identical typed operations with separately
authorized human and machine endpoints.

Inspect both ELF segments, allocated sections, entry/controlled-fault symbols,
load byte identity, and absence of dynamic relocation or W+X state. Verify
normal-image exclusion and combined-feature precedence. Native smoke must
show repeated interactions and presentations, exact captured pixels, contained
service failure with resource restoration, and a working bootstrap fallback.
Run the existing full validation matrix and exercise VirtualBox if supported.

Reject a shortcut that puts window policy in the kernel, treats IDs or origin
labels as authority, paints directly into the framebuffer from user space,
exposes an incomplete candidate as committed state, weakens old proof
contracts, or advertises private app/agent functionality from public demos.

## Production follow-on

This slice deliberately does not complete application surface admission,
client task/channel grants, revocable input focus and capture, process
supervision/restart, service-loss recovery, a semantic IPC API for agents,
private content disclosure, text input/IME, pointers/touch, multiple outputs,
DPI scaling, damage tracking, GPU acceleration, persistent configuration,
package/extension loading, accessibility transport, or performance targets.
Those capabilities should extend the shared canonical model and narrow native
mechanisms, with measured fault and privacy behavior at each boundary.
