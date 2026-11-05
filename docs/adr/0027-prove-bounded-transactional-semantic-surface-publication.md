# ADR 0027: Prove bounded transactional semantic-surface publication

- Status: Accepted
- Date: 2026-09-19

Anomalous implements one separate host-only proof in which an exact canonical
desktop successor remains semantically private while one renderer attempt at a
time reads an immutable redacted snapshot. The service publishes that
revision's human, accessibility, and machine semantics together only after
consuming an exact trusted presentation receipt. One renderer may be replaced
once during the surface lifetime without publishing its abandoned work.

The proof does not implement or validate a renderer, transport pixels, define a
window or input protocol, create a stable ABI, or enter the normal image.

## Context

ADR 0025 accepts one host-only canonical semantic scene. Each committed scene
revision derives an accessible render plan, accessibility tree, and machine
projection together. Its replacement protocol keeps a prepared revision private
until one atomic commit, and protected source labels become one fixed redaction
in both projections.

ADR 0026 proves that one generated redacted capsule can drive a separately
linked supervised renderer. Its first renderer generation faults after staging
half a frame, its replacement stages a complete frame, and only that complete
replacement publishes. The capsule is fixed at build time. ADR 0026 explicitly
leaves live revision transfer, freshness, ownership, backpressure, policy, and
failure behavior to a later decision.

A live desktop adds a temporal boundary that neither proof settles. If semantic
revision N+1 becomes public when a renderer merely accepts it, a later renderer
fault can leave machine and accessibility clients observing N+1 while people
still see revision N. If renderer dequeue releases producer capacity, its crash
can lose the only owned candidate. If recovery reuses the same endpoint, lease,
or renderer generation, delayed work can masquerade as the replacement. If a
service silently overwrites or coalesces a candidate, it can lose private
ownership and conceal backpressure policy.

This proof isolates those host-side transaction obligations before adding
native IPC, framebuffer publication, windows, focus policy, input, action
effects, or renderer correctness. It consumes a deliberately trusted host
assertion that presentation completed rather than pretending a host model has
verified pixels or scanout.

## Decision

### Add a separate host-only desktop-surface service

Add an `anomalous-desktop-service` crate beside
`anomalous-desktop-scene`. It is `no_std`, allocation-free, unsafe-free, and
depends only on the accepted scene crate. It is not linked into the kernel,
`anomalous-abi`, a vessel, a renderer image, ADR 0024's display operation, or
the normal boot image. It defines no serialized representation or compatibility
ABI.

One move-only `desktop_surface` owns one exact ADR 0025 scene incarnation, its
scene control, one fixed logical `surface_identity`, one current
`renderer_identity`, a nonzero `publication_sequence`, and at most one pending
successor. The public surface numbers identify an intended host presentation
target but grant no access to a display, framebuffer, or presenter.

At every live point the service has:

- exactly one complete committed revision exposed through its paired human,
  accessibility, and machine projections; and
- at most one complete prepared successor that is not exposed through those
  public semantic projections.

It has no queue beyond that successor slot. `desktop_surface`,
`renderer_endpoint`, `render_lease`, `prepared_presentation`, and related names
are conventional descriptions of this bounded proof. It introduces no new
occult platform abstraction or nomenclature entry.

### Extend ADR 0025 only with a borrowed prepared preview

The service must derive renderer data before committing the scene. ADR 0027
therefore adds a narrow API to the ADR 0025 crate without changing its committed
scene, projection, action, focus, or replacement semantics:

- `prepared_revision::preview()` returns a borrowed
  `prepared_render_preview` containing the prepared identity, base and next
  revisions, state, viewport, authority-free paint commands, and already
  projected labels; and
- `prepared_revision::into_candidate()` recovers the complete candidate from a
  preparation still owned by its caller.

The preview cannot commit, mint or resolve action or focus authority, expose a
projection owner, or reveal protected source content. Candidate recovery does
not cancel a successor already installed in `desktop_surface` and does not
mutate a live scene. ADR 0025 otherwise remains unchanged.

### Fix one exact desktop-service profile

The service intentionally accepts a strict subset of ADR 0025's possible label
values. Its single profile has:

- one 1024x768 logical root surface;
- the exact eight-node, two-action ADR 0025 topology, roles, parents,
  identities, and geometry;
- the exact declared-public labels `Anomalous desktop`, `Anomalous`,
  `Navigation`, `Open shell`, `Inspect system`, `Workspace`, and
  `System workspace` in slots zero through six;
- either the declared-public status `System ready` or any valid protected
  status source in slot seven, with protected source projected only as
  `Protected status`;
- exactly 25 paint commands for standard depth or 21 for flat depth; and
- the complete ADR 0025 state matrix: 64 valid states and 32 invalid states
  that focus a disabled action.

Variable labels, topology, geometry, text rendering, localization, arbitrary
command counts, or a larger surface require a later profile and evidence.

### Bootstrap one already-presented revision

`desktop_surface::new_presented` consumes the initial candidate and returns the
surface owner, one move-only `surface_control`, one move-only
`presentation_control`, and one move-only `renderer_endpoint`. Construction
starts at scene revision one and publication sequence one. The trusted host is
asserting that this initial revision is already fully presented together with
its public semantic projections.

Construction validates the exact service profile and the complete ADR 0025
scene. Rejection returns the candidate owner. This bootstrap does not inspect a
framebuffer, authenticate the host, or prove that initial pixels match the
candidate.

Successful construction allocates one unexposed, nonzero, nonwrapping,
process-local service brand. Equal public scene, surface, renderer, revision,
sequence, or attempt values in separately branded owners grant no cross-owner
authority. The proof makes no safe-API claim that the process-wide private
service-brand allocator can be driven to exhaustion in a test.

### Separate surface, presentation, and renderer authority

The authorities have disjoint jobs:

- `surface_control` may submit and publish a successor, authorize the one
  renderer replacement, and retire the whole surface;
- `presentation_control` may attest that one exact prepared presentation has
  crossed the trusted external presentation boundary; and
- `renderer_endpoint` may acquire only the exact pending snapshot for its
  privately branded service and current renderer generation.

No control, endpoint, lease, prepared presentation, or presentation receipt is
constructible from public scalar fields. `restart_receipt` owns the replacement
endpoint and releases it only by value. The service exposes no owned
`desktop_scene`, `scene_control`, prepared scene, projection owner, local
action/focus owner, renderer endpoint, or service control from its internal
storage through an escape hatch.

Owner-preserving rejection envelopes are an intentional exception to
content-free diagnostics: construction and submission rejection return the
caller's complete `scene_candidate`, including its protected source owner, for
retry or explicit clearing. Borrowing or consuming that returned owner does not
expose service-owned state. Its separate `surface_error` remains a static,
content-free code.

Public scene, surface, renderer, revision, publication-sequence, attempt, node,
action, rectangle, label-selector, and receipt fields remain correlation data,
not authority.

### Use five explicit live phases

The service exposes this bounded lifecycle:

1. `steady`: only the committed revision exists;
2. `queued`: one complete successor and immutable snapshot occupy the sole
   pending slot;
3. `leased`: one exact renderer attempt may read that snapshot;
4. `ready`: consuming the lease produced one move-only
   `prepared_presentation`; and
5. `presented`: consuming that prepared presentation at the trusted host
   boundary produced one move-only `presentation_receipt`, but the semantic
   revision is still not committed.

The ordinary success path is:

`steady -> queued -> leased -> ready -> presented -> steady`.

The only renderer-replacement edge is `leased|ready -> queued`. The receipt-held
`presented` phase cannot restart. There is no pending-revision cancellation
transition. Successful publication or explicit whole-surface retirement is the
only way to release an occupied slot.

Dropping a lease, prepared presentation, presentation receipt, endpoint, or
control performs no protocol transition. In particular, dropping a
presentation receipt strands the surface in `presented` until explicit
retirement. The host has already asserted external presentation at that point;
this proof cannot withdraw or clear those external pixels and cannot preserve
human/semantic alignment if the host loses the receipt instead of publishing.

### Submit one complete successor or return exact backpressure

`desktop_surface::submit_revision` consumes the candidate definition owner,
checks the exact surface control and committed base revision, preflights the
next publication sequence, validates the exact service and ADR 0025 profiles,
prepares both semantic projections, and derives the renderer snapshot before
changing service state.

If the surface is `steady`, success installs the complete prepared owner and
snapshot, enters `queued`, and returns a move-only `queued_revision` describing
the accepted base and next revision. The receipt is descriptive only and is not
accepted as authority by another operation.

If any pending phase is occupied, submission returns `pending_full` plus the
complete candidate owner. It does not replace, drop, coalesce, reorder, or
compare the candidate; advance a revision, renderer generation, attempt, or
publication sequence; change the current projections; mint a lease; or alter
the existing pending owner. Malformed, invalid, stale, foreign, exhausted, and
wrong-control submission likewise returns candidate ownership and leaves the
service unchanged.

The caller may retry only while retaining its private owner. If another
revision publishes first, the candidate must be submitted again against the
fresh committed base.

### Derive one immutable bounded redacted renderer snapshot

Every queued revision retains one immutable private `render_snapshot` whose
complete Rust value is statically asserted to be at most 1,024 bytes. Its
fixed-size arrays and the accepted scene profile make an oversized runtime
snapshot unrepresentable; acceptance therefore proves the compile-time cap,
both 21- and 25-command cases, and checked out-of-range access rather than
claiming a runtime oversized-snapshot rejection.

The snapshot contains only the renderer-facing subset of the prepared preview:

- public scene and surface correlation identities, base and next revisions,
  and declared scene state;
- canonical command order and command, shape, and semantic-color tokens;
- checked logical rectangles and canonical slot binding;
- active-state and visible-focus drawing data; and
- one of nine fixed label selectors for the eight accepted labels and protected
  placeholder.

It omits protected source bytes and exact length, private scene and service
brands, construction controls, projection owners, local action and focus
handles, machine-only action descriptors, renderer authority, and presentation
authority. Focus shape is drawing data, not a session route or focus authority.
A node, action, label, coordinate, or snapshot field cannot invoke an effect.

The representation is private local Rust data. It has no magic, byte decoder,
format negotiation, stable layout, media type, file extension, or wire-version
promise. Tests compare the complete defined typed-field view, not raw object
bytes or padding. A future native transport must define its own copied,
versioned boundary rather than serialize this Rust value by convention.

### Lease one attempt and return one prepared presentation

The exact current renderer endpoint may acquire the queued snapshot once.
Success enters `leased` and returns one move-only `render_lease` bound through
the private service brand to the exact surface, pending revision, renderer
generation, delivery attempt, and stored snapshot.

The lease offers explicit borrowed access to the immutable redacted snapshot.
Repeated reads expose the same complete typed fields. It grants no access to
either canonical semantic projection and no authority to submit, publish,
restart, attest presentation, or retire.

Duplicate acquisition, wrong endpoint, foreign service, stale renderer
generation, wrong lifecycle phase, or mismatched delivery owner rejects without
changing public semantics or the pending transaction. Consuming the exact lease
through `complete_render` enters `ready` and returns one move-only
`prepared_presentation`. Readiness changes no public semantic state and proves
neither renderer integrity nor presentation.

Delivery attempts are local to one pending transaction. The ordinary attempt
is one; after the single authorized restart the replacement attempt is two.
The revision, renderer generation, and private brand remain part of the exact
binding, so an equal attempt scalar from another transaction grants nothing.

### Permit exactly one renderer replacement

The exact surface control may record renderer failure only in `leased` or
`ready`, before a presentation receipt exists. Restart is rejected in `steady`,
`queued`, and `presented`.

The one successful replacement advances the public renderer generation without
wrapping, invalidates the old generation before returning the new endpoint,
preserves the same immutable pending snapshot and prepared scene, and returns to
`queued`. The old endpoint, lease, or prepared presentation remains stale even
if its public numeric fields match. The committed revision and public semantic
projections remain unchanged.

The restart budget is exactly one for the entire surface lifetime. A later
attempt to replace the renderer returns `restart_limit_reached`; renderer
generation exhaustion returns `renderer_generation_exhausted`. Both failures
leave the held owner and service state unchanged. There is no third renderer,
automatic retry, or implicit cancellation.

This host proof invalidates service-side authority only. It does not withdraw an
external task, clear its memory, or prove that a failed renderer stopped
executing.

### Publish only at the trusted presentation-receipt boundary

`desktop_surface::attest_presented` takes the exact
`prepared_presentation` by value under `presentation_control`. Success changes
`ready` to `presented` and returns one move-only `presentation_receipt` bound to
the service, surface, pending revision, renderer generation, and attempt.
Foreign or stale prepared presentations return their owner and leave the phase
unchanged.

This operation is the deliberately named trusted-host boundary. It does not
inspect a framebuffer, authenticate the asserting host, verify a renderer,
prove that pixels match the snapshot, or establish scanout completion. A future
native presenter must mint equivalent evidence at its actual operation boundary
rather than trust a caller-selected scalar or receipt-shaped value.

`desktop_surface::publish` requires the exact surface control and consumes the
presentation receipt. It validates every public and private binding, the
`presented` phase, the unchanged committed base, and a nonwrapping next
publication sequence before attempting the stored ADR 0025 commit. Wrong,
stale, foreign, or mismatched owners return the receipt and leave both the
committed and pending state unchanged.

The stored scene commit is expected to succeed because the service exclusively
owns the exact scene and prepared owner. The lower-layer API remains fallible,
however. If it reports an invariant rejection, `publish` restores the complete
prepared owner, snapshot, phase, and attempt count, returns the presentation
receipt, and reports static `invariant_violation`; no public semantic state or
publication sequence changes.

Successful publication:

1. commits the prepared ADR 0025 revision;
2. makes its accessible render plan, accessibility tree, and machine
   projection public together;
3. advances the nonwrapping `publication_sequence`;
4. consumes the presentation receipt exactly once; and
5. clears the pending slot so producer capacity becomes available.

`publication_receipt` is a move-only descriptive result containing the surface,
new revision, publication sequence, renderer, and attempt. No operation accepts
it as authority.

Readers observe either the complete old semantic revision or the complete new
semantic revision, never a prepared or mixed projection pair. This is atomic
host semantic publication after a trusted presentation assertion. It is not
simultaneous hardware scanout, cross-process memory ordering, a page flip, or
proof that an external display contains correct pixels.

### Preserve current action and focus authority until publication

The service does not expose ADR 0025 projection owners directly. It provides a
narrow branded facade that can mint action and focus handles from the currently
committed accessible or machine projection, validate one action through
`resolve_action`, and validate canonical focus traversal through
`traverse_focus`.

These operations perform no action effect, mutate no canonical scene focus,
route no input, and grant no application capability. Handles for revision N
remain valid while N+1 is queued, leased, ready, or presented. They become stale
only after successful publication commits N+1. Foreign equal-public-ID handles
remain invalid and rejection returns the move-only handle owner.

### Keep protected content and diagnostics bounded

Protected source bytes, exact length, excerpt, and digest never enter the
renderer snapshot, scalar `surface_error`, endpoint, lease metadata, prepared
presentation, presentation receipt, or publication receipt. Two otherwise
identical candidates with different valid protected status values and lengths
derive equal complete typed snapshot fields and equal public projections under
the same public fixture.

This claim does not hide activity metadata. Submission, backpressure, leasing,
readiness, replacement, presentation attestation, publication, and retirement
are visible through direct call results and public correlation fields such as
phase, revision, sequence, renderer, attempt, count, and geometry. A
protected-only candidate may therefore be externally distinguishable as an
operation even though its value and exact length are absent. This proof adds no
padding, batching, traffic shaping, private revision channel, or activity,
cadence, volume, resource-pressure, or timing noninterference claim.

Errors format only as static content-free codes. Candidate-returning rejection
envelopes intentionally carry the caller's original private owner but implement
no implicit formatting or byte/string conversion. The crate defines no ambient
observation, log, telemetry sink, trace, screenshot, crash artifact, or history.
Its direct read APIs are local typed inspection boundaries, not authorization
to persist or transmit their results.

Owned scene text follows ADR 0025's logical clearing path when an owner is
dropped or explicitly cleared. This is not secure erasure of caller copies,
compiler temporaries, caches, swap, devices, external renderer memory, or
microarchitectural state.

## Required invariants

- One privately branded live `desktop_surface` and its move-only controls govern
  each service incarnation; public data never confers authority.
- The service exposes exactly one complete committed semantic revision and at
  most one complete prepared successor.
- A queued, leased, ready, or presented successor is never exposed through the
  public human, accessibility, or machine projections.
- Full pending capacity returns the complete candidate owner with no implicit
  overwrite, drop, coalescing, reordering, or unbounded queue.
- Each immutable renderer snapshot is compile-time bounded to at most 1,024
  bytes and contains only accepted public drawing data or fixed redaction.
- The exact endpoint, lease, prepared presentation, presentation receipt,
  service brand, scene, surface, revision, renderer generation, and attempt
  agree before their transitions.
- Renderer readiness cannot publish. Only the exact trusted presentation
  receipt can make the complete paired semantic revision public.
- One controller-authorized replacement from leased or ready preserves the same
  private candidate under a fresh endpoint and second attempt. Old-generation
  work remains stale; queued and presented phases cannot restart; no second
  replacement is created.
- A revision, publication sequence, renderer generation, lease, prepared
  presentation, or presentation receipt is never accepted twice and no
  service-advanced freshness value wraps.
- Protected values and lengths affect no snapshot or public projection content,
  while operation activity and cadence remain explicitly observable.
- Rejection is failure-atomic and returns every caller-owned private owner needed
  for retry, failure handling, or retirement.
- Surface-control retirement remains available after freshness failure, drops
  any pending and committed owners through their logical clearing paths, and
  creates no successor or public revision.
- The crate adds no native path and remains absent from the kernel, ABI, feature
  images, and normal image.

## Non-goals

This decision does not define or implement:

- a renderer, rasterizer, pixel oracle, framebuffer presenter, display
  transport, shared memory, kernel rite, native IPC, vessel, or normal-image
  runtime;
- renderer integrity, correct pixels, scanout completion, tear-free display,
  page flipping, GPU commands, DMA, fences, vsync, or color management;
- a compositor, window or surface protocol, z-order, occlusion, clipping,
  damage, application surface, multiple display clients, or display manager;
- input routing, focus arbitration or mutation, action effects, hit testing,
  secure attention, consent, authentication, authorization policy, clipboard,
  drag and drop, or application launch;
- a general scene graph, arbitrary topology, variable commands or labels,
  layout, fonts, shaping, localization, scaling, animation, themes, or multiple
  displays;
- concurrency, threads, SMP ordering, distributed revocation, fairness,
  deadlines, producer admission, or denial-of-service containment;
- more than one pending revision, more than one active renderer, more than one
  restart per surface lifetime, automatic retry, latest-wins behavior,
  cancellation, or persistence;
- protected activity, cadence, volume, timing, resource-pressure, or side-
  channel noninterference; secure erasure; or crash-safe host persistence;
- telemetry transport, trusted audit provenance, screenshot or capture
  authority, Oracle or Eidolon integration, or retention policy;
- serialization, a wire format, format negotiation, stable scene/snapshot/
  surface/graphics ABI, or compatibility promise; or
- a new occult platform abstraction or nomenclature term.

## Acceptance evidence

Focused host evidence passed:

- 19 `anomalous-desktop-scene` unit tests, including the borrowed prepared
  preview and candidate-recovery path;
- 136 scene documentation/compile-fail tests and warning-denied scene Clippy;
- 14 `anomalous-desktop-service` unit tests;
- 74 service documentation/compile-fail tests and warning-denied service
  Clippy;
- all 64 valid constructors and all 32 invalid constructors with candidate
  return;
- all 32 invalid successor states with complete owner return and unchanged
  committed observation;
- all `64 * 64 = 4,096` valid state transitions, checking that the committed
  typed observation remains unchanged through queued, leased, ready, and
  presented and changes only after publication;
- fixed-profile, foreign-scene, strict-backpressure, stale-generation,
  equal-public-ID/different-brand, publication-sequence exhaustion, and
  renderer-generation exhaustion rejection paths;
- repeated immutable snapshot reads, typed-field equality across the restart,
  one fresh endpoint and attempt, stale old owners, second-restart rejection,
  and restart rejection after presentation attestation;
- protected sources of different lengths producing equal complete typed
  snapshot and public-projection fields, with the activity/revision non-claim
  demonstrated separately;
- committed action/focus validity before publication and staleness afterward;
- directed steady, queued, leased, ready, and presented lifecycle rejection and
  retirement coverage; and
- compile-fail coverage for move-only controls, endpoints, leases, prepared
  presentations, receipts, semantic handles, service owners, and borrowed
  snapshot escape or implicit content conversion.

This is directed bounded lifecycle coverage, not a claim of exhaustive model
checking or a counted enumeration of every possible API call sequence.
Presentation-receipt uniqueness is enforced by move-only construction and
consumption; runtime tests cover stale and foreign owners rather than fabricating
a duplicate receipt through safe Rust. Public scalar constructors cover zero,
maximum, and nonwrapping identity behavior, but the proof does not claim a safe
API test of process-wide private brand exhaustion or impossible third-attempt
construction.

The complete acceptance run also passed:

- `make check`, including formatting, the complete configured host suite,
  warning-denied host and freestanding x86-64 Clippy, 329 all-feature kernel
  tests, 56 xtask tests, renderer base and scene-conformance tests, artifact
  inspection, the all-feature freestanding link, script syntax checking, and
  `normal-exclusion`;
- the full established QEMU regression matrix, run serially: `smoke`,
  `fault-smoke`, `circle-smoke`, `control-smoke`, `rite-smoke`,
  `scheduler-smoke`, `supervision-smoke`, `shell-vessel-smoke`, both configured
  `memory-smoke` sizes, `interrupt-smoke`, `input-wait-smoke`, `shell-smoke`,
  `display-vessel-smoke`, and `supervised-renderer-smoke`;
- a final `normal-exclusion` run after the QEMU matrix; and
- `cargo tree -p kernel`, which contains no `anomalous-desktop-service`
  dependency.

Implementation checkpoint `2076eba` contains the service proof. VirtualBox was
not run. No acceptance result claims that this host-only service executed in
QEMU; those emulator runs are unchanged native regressions.

## Consequences

Anomalous gains a falsifiable transaction boundary between canonical semantic
preparation and an external presentation mechanism. Producer pressure is
explicit, renderer failure cannot expose a prepared semantic revision, and one
fresh renderer may retry the same private candidate without making old work
valid again. Human, accessibility, and machine semantics become public together
only after the trusted host reports completed presentation.

The proof also exposes the remaining gap honestly. Its receipt is a trusted host
assertion, its renderer snapshot is a local private representation, and its
renderer may produce incorrect or deceptive output. A later native decision
must define copied transport, task and surface authority, actual presentation
evidence, supervision composition, and privacy policy without turning this host
model into a stable ABI by accident.

The strict one-pending slot intentionally trades throughput for tractable state
and exact ownership. General queues, coalescing, cancellation, parallel
rendering, and latency policy need separate evidence.

## Alternatives considered

### Commit semantics when the renderer dequeues

Rejected. A renderer fault after dequeue could expose the new machine and
accessibility state while the old human frame remains visible, and it could
release producer capacity before presentation owns the candidate.

### Treat renderer readiness as presentation

Rejected. Readiness is a renderer-controlled lifecycle claim. It proves neither
correct pixels nor completed presentation and cannot replace a trusted
operation-boundary receipt.

### Queue or coalesce multiple prepared revisions

Deferred. FIFO, latest-wins, cancellation, coalescing, memory budgets, and
protected activity leakage are distinct policies. One occupied slot and exact
ownership return make this proof bounded.

### Give the renderer the canonical scene owner

Rejected. It would move scene control, machine/action metadata, protected source
ownership, and publication authority into a component that needs only a
redacted render projection.

### Use revision numbers or a snapshot digest as authority

Rejected. Both are copyable correlation data. A digest may help compare bytes
but cannot prove holder, freshness, presentation, or authorization.

### Define the native scene/surface wire now

Deferred. A host-only state machine first proves transaction ownership,
backpressure, retry, and semantic publication without simultaneously adding a
kernel operation, task grants, shared memory, parser, or stable ABI.

### Introduce an occult name for the service

Rejected. This is a bounded composition of existing scene, service,
supervision, and presentation concepts, not a new platform-level abstraction.
Conventional searchable terminology is more precise.
