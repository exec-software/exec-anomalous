# ADR 0038: Add native window content surfaces

- Status: Proposed
- Date: 2026-09-26

## Context

The native workspace can arrange and decorate windows, but their contents are
built into the compositor. The next development step is an explicit path from
caller-rendered pixels to a particular window. This should preserve the shared
semantic model, exact window identity, granted authority, and complete-frame
publication already used for window actions. It must not make arbitrary pixels
an implicit source of authority or claim a semantic tree that they do not have.

The user intends to begin TRI-ABI development with Linux once this rendering
foundation is available. Proposed [ADR 0008](0008-isolate-foreign-platform-compatibility.md)
keeps Linux, Windows, and macOS veils independent. Linux-first defines the
implementation order. macOS has its own Darwin/XNU foundation, combining Mach
and BSD components, as described in
[Apple's XNU source](https://github.com/apple-oss-distributions/xnu). Each veil
needs its own execution and graphics contracts even when it can reuse native
rendering facilities.

A graphics buffer path and a foreign execution ABI are separate responsibilities.
For example, Wayland specifies shared-memory pools, pixel-buffer dimensions and
stride, and surface attachment/commit/release; these are distinct from loading
an executable and providing its syscalls and runtime. The
[official Wayland protocol](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_shm)
is a reference for separating responsibilities, not a protocol implemented by
this proposal. There is no Linux ELF loader, Linux syscall personality, Wayland
server, X11 server, macOS loader, or foreign framework in this increment.

## Decision

Add a portable `no_std`, allocation-free, unsafe-free
`anomalous-window-surface` library. Its initial profile is opaque RGB8 with
explicit width, height, stride, and exact backing length. Dimensions are
nonzero and bounded by 1024×768; stride is at least three bytes per pixel and
at most 4096 bytes. Backing length must equal `stride * height`, including
padding after the final row. Validate the complete layout before exposing
pixels, use checked arithmetic, and reject insufficient or excess storage.
`frame::row` excludes padding from visible content.

A frame borrows immutable caller-owned storage. Its validated descriptor is
small, copyable public metadata, including a printable ASCII public label of
1–64 bytes and a process-local unique nonwrapping frame identity.
`frame::try_new` mints identity only after complete validation. The label is
deliberately public; no private text,
application secrets, or pixel bytes enter the canonical copyable desktop
snapshot. Identity identifies a frame under the local contract; it is not a
cryptographic content hash or authentication of an external producer.

Provide a caller-owned canvas with bounded clear, filled-rectangle, and line
operations plus deterministic demonstration scenes. After the mutable canvas
borrow ends, `frame::try_new` creates an immutable borrow for composition.
The caller retains ownership of the
storage, and Rust borrowing prevents mutation while that frame is in use. No
shared-memory mapping or untrusted producer runs concurrently with the
compositor in this slice.

Extend each canonical content-bearing window with the exact surface descriptor
and a nonwrapping `surface_sequence`. Human, accessibility, and machine
observations share the same window geometry, descriptor, public label, content
sequence, and desktop revision. This describes the raster surface and the
fixed demonstration; it does not make arbitrary raster controls, text, or
application documents semantically accessible.

A distinct `present` grant authorizes content updates. The trusted controller
uses `desktop::bind_surface` with its `desktop_control` and an observe/present
granted client endpoint to issue a move-only `surface_owner` bound to that
client generation and one exact Graphics Lab window incarnation. An owner for a closed window must not authorize its
replacement, even when the new window reuses a slot. A fresh client generation
must not use a former client's owner. The owner conveys no arrange, manage,
customize, or unrelated-window authority.

`prepare_surface` requires the owner's identity, current granted client, exact
committed desktop revision, an admissible next descriptor, and an available
successor slot. It prepares one pending update and uses the existing
presentation acknowledgement before publishing its metadata or advancing its
content sequence. Rejected authority, stale state, exhausted counters, invalid
content, and backpressure leave committed state unchanged. A descriptor alone
cannot bypass the owner or the pending-transaction discipline.

Opening Graphics Lab supplies its initial descriptor through
`open_graphics(descriptor)` so its window and first content become visible in
one transaction. The initial creation requires the corresponding management,
arrangement, and presentation grants. Subsequent content replacements use the
window-specific owner and the ordinary exact-revision preparation path.
Reopening an existing Graphics Lab brings it to the active workspace, restores
and focuses it, and retains its current content rather than resetting the scene.

Before beginning an external frame presentation, validate the complete bounded
set of at most four immutable `surface_binding` records against the candidate
view. Each binding names an exact window and must match that window's complete
descriptor. Every attached live window requires a binding, including minimized
windows and windows on another workspace. Missing,
duplicate, stale, unrelated, or mismatched bindings must not cause a partially
presented frame. After the presentation gate begins, reborrow the same retained
immutable frames; keep those bytes alive and unchanged through all 768 output
rows and the existing acknowledgement/publication boundary. `composition_frame`
retains immutable view and frame borrows for row composition. The prior free
`compose_row` rejects a view with attachments when no bindings were supplied.
Validation is not permission to fetch a different buffer later.

The compositor samples each surface into the shared public `client_viewport`
using nearest-neighbor stretching. It clips to that client area and the output
row, retains ordinary window z-order and occlusion, and draws desktop-owned
chrome and the pointer through their existing paths. A content producer cannot
overwrite another window, a titlebar control, panel, or cursor through a larger
buffer or different stride. Resizing changes the sampling destination while
retaining the same source frame; this profile does not negotiate application
resize events or make a new content revision merely for geometry changes.

The native demonstrator is a singleton Graphics Lab inside the existing trusted
workspace vessel. **F2** opens or restores it; **F3**, while Graphics Lab is
focused, replaces its scene through the granted content-update path. At service
startup the canvas draws two 96×64 RGB8 caller-owned buffers once: a gradient
with orange/teal geometry and a chart with five bars and a rising line. Both
frames remain immutable for the session; F3 alternates their descriptors and
advances the canonical content sequence after presentation. This demo does not
redraw or animate its buffers on each keypress. The reusable drawing API can
produce other content under the same immutable-borrow contract. The producer
and compositor still share one trusted owner. There is no separately scheduled
application, task-grant transport, shared buffer mapping, or new native rite.
The kernel continues to receive the existing complete display rows and owns
framebuffer access, verification, containment, and resource reclamation.

## Alternatives and consequences

Adding another hardcoded compositor window would not establish reusable
application content. Giving a producer framebuffer access would bypass window
clipping and desktop ownership. A foreign display protocol now would also need
transport, external producer identity, lifetime management, scheduling, and
process isolation that this desktop does not yet supply.

Borrowed software surfaces establish the small rendering boundary first. The
copyable descriptor deliberately excludes bytes; actual immutable frames are
retained separately by the native owner. Full-frame composition and verified
presentation remain the cost of each accepted update. Nearest-neighbor
stretching is deterministic and bounded but can distort aspect ratio and
exposes low-resolution pixels when enlarged. Alpha, damage regions, GPU
resources, zero-copy sharing, present fences, scanout completion, application
input delivery, private-content policy, and a general accessible content tree
require separate follow-up contracts.

This is an internal native Rust interface under a proposed ADR, not a stable
wire ABI. TRI-ABI adapters may eventually translate their own window and buffer
operations into a versioned successor, but Linux executable/runtime work and
macOS/Windows compatibility remain independent milestones. No compatibility
success follows merely from displaying a native buffer.

## Validation, disproof, and reversal

Require boundary tests for dimensions, stride, exact backing length, padding,
clipped drawing, extreme line coordinates, immutable borrowing, unique identity,
and counter exhaustion. Workspace tests must reject missing grants, mismatched
client generations, wrong window incarnations, stale revisions, invalid update
order, and a second pending successor without changing committed state. Verify
that geometry changes preserve content and that failed/aborted presentation
cannot publish replacement metadata.

Composition tests must reject invalid binding sets before output begins and
cover exact pixels for padded rows, scaling, clipping, overlap, minimize,
workspace visibility, resize/maximize, and preserved chrome. The native proof
must exercise initial opening, scene replacement, retained content across
window operations, close/reopen identity, complete acknowledged frames, and
existing failure containment and reclamation. Run the complete repository
validation matrix and record actual native evidence separately in the notebook.

A changed frame during composition, content leaking beyond its viewport,
stale owner updating a new incarnation, public revision preceding its complete
frame, or missing metadata parity disproves the implementation. Reversal removes
the Graphics Lab adapter and optional content fields/bindings while retaining
the prior native windowing path. It does not alter accepted fixture ADRs or
require a foreign ABI decision.
