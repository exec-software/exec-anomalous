# ADR 0025: Prove a bounded canonical semantic desktop scene

- Status: Accepted
- Date: 2026-09-19

This record accepts one separate host proof in which a single immutable
desktop-scene revision derives an accessible human render plan, a first-class
accessibility tree, and a machine-readable semantic projection together. The
proof has eight exact nodes, two actions, fixed logical geometry, explicit
label disclosure, and a complete bounded state matrix. It does not define a
compositor, window or surface protocol, input router, action executor, stable
ABI, or normal-image desktop runtime.

## Context

ADR 0001 requires people and machines to consume projections of one typed
system model rather than reconstructing one another's interfaces. ADR 0007
requires visual depth to remain subordinate to labels, borders, focus,
keyboard operation, semantic accessibility, and nonvisual state. ADR 0024 now
proves that a fixed ring-3 task can present validated pixels without receiving
the framebuffer mapping, but explicitly leaves authoritative desktop state
above that raw presentation boundary.

The next slice must prevent three architectural traps. A renderer-owned object
tree would tempt machine clients and assistive technology to scrape pixels or
private renderer state. A separately authored accessibility sidecar could
drift from visible controls and actions. A machine-only object API could expose
actions or authority that people cannot discover and inspect.

This first scene proof should remain finite enough to exhaust. It must not hide
a general widget toolkit, layout engine, window manager, or policy service
inside a crate whose evidence covers only one fixture.

## Decision

### Add a separate host-only scene proof

Add a dependency-free `anomalous-desktop-scene` crate beside the existing
session proofs. It is `no_std`, allocation-free, and unsafe-free. It is not
linked into the kernel, `anomalous-abi`, ADR 0024's ring-3 image, a vessel, or
the normal boot image, and it defines no serialized or stable format.

`desktop_scene` and related names are deliberately conventional. This proof
does not introduce a new occult platform abstraction.

The crate owns one exact logical scene with a 1024x768 viewport and these slots
in canonical preorder:

| Order | Slot | Role | Parent | Logical rectangle `(x, y, w, h)` |
| ---: | --- | --- | --- | --- |
| 0 | root | desktop | none | `(0, 0, 1024, 768)` |
| 1 | masthead | region | root | `(32, 24, 960, 88)` |
| 2 | navigation | region | root | `(32, 136, 256, 600)` |
| 3 | primary action | control | navigation | `(56, 184, 208, 64)` |
| 4 | secondary action | control | navigation | `(56, 272, 208, 64)` |
| 5 | content | region | root | `(312, 136, 680, 600)` |
| 6 | heading | heading | content | `(352, 176, 600, 48)` |
| 7 | status | status | content | `(352, 248, 600, 96)` |

Every slot has fixed checked integer geometry and one nonzero generational node
identity. The primary and secondary controls have exactly one nonzero
generational action identity each. Roles, parents, order, geometry, and action
kinds are validated rather than inferred from labels or pixels. The maximum
tree depth is three levels including the root.

This is an exact fixture schema, not a general tree API. Replacing it with a
variable scene graph requires a later decision and new resource, topology, and
denial-of-service evidence.

### Exhaust one bounded state space

The candidate state contains:

- enabled or disabled state for each of the two controls;
- focus at none, primary, or secondary;
- a declared-public or protected status label;
- standard or flat neumorphic depth; and
- standard or reduced motion preference.

Those dimensions form the complete declared UI-state matrix of exactly
`4 * 3 * 2 * 2 * 2 = 96` candidates. Exactly 64 are valid. The other 32 focus
a disabled control and must reject. Construction therefore publishes no scene;
replacement preparation must leave the existing committed scene unchanged.
This does not exhaust labels, identities, revisions, or malformed definitions.
Motion never produces an animation command in version 1, so corresponding
commands and nodes have equal exposed fields; only the explicit motion value in
their projection context differs.

Focus is recorded canonical scene state, not focus discovery or arbitration.
It is distinct from ADR 0023's session-route focus and cannot select an input
route, authenticate a person, prove presence, or authorize an action.

### Commit a complete revision atomically

Construction allocates an unexposed, nonzero, nonwrapping process-local
instance brand and returns one move-only scene owner plus one move-only scene
control. The public generational `scene_identity` remains correlation data. A
trusted host must reserve its complete lineage; equal public identities in two
live owners confer no cross-owner authority.

The initial exact definition becomes committed revision 1 only after complete
validation and projection derivation. Later replacement has two phases:

1. `prepare_revision` verifies the exact control and base revision, preflights
   the next nonwrapping revision, validates the complete candidate, derives all
   bounded outputs, cross-checks their semantic equality, and returns a
   move-only prepared revision. Preparation does not mutate the live scene and
   exposes none of the uncommitted projections.
2. `commit_revision` revalidates the private brand, scene identity, exact base
   and next revisions, and committed lifecycle before one infallible swap. The
   new canonical definition and both precomputed projections become visible
   together. Preparation rejection, including malformed or exhausted input,
   returns the candidate owner. Commit rejection returns a stale, competing,
   or foreign prepared owner. Both leave the prior revision unchanged.

Competing preparations may share a base revision; exactly the first valid
commit succeeds. The scene revision never wraps. Consuming retirement remains
available at the maximum revision without incrementing it, invalidates local
handles before clearing owned label storage, and permits replacement only by
constructing a new privately branded owner. The trusted host remains
responsible for reserving and advancing the public scene-identity lineage; the
crate does not reject equal public identities across separately branded owners.

Logical clearing is an owned-storage lifetime rule, not secure erasure of
caller copies, compiler temporaries, caches, swap, or hardware.

### Derive human, accessibility, and machine projections together

Every committed scene owns one precomputed projection pair. Borrowing it gives:

- an `accessible_render_plan` containing fixed semantic paint commands and an
  eight-node accessibility tree; and
- a `machine_scene_projection` containing the same eight nodes plus two ordered
  action descriptors.

Structural records and paint commands are precomputed. Both label APIs borrow
through one shared projection function from the same immutable committed
definition; they do not keep independently mutable or separately authored
label copies that could drift.

Both carry the exact scene identity, revision, fixed viewport, depth and motion
context, canonical node order, parentage, roles, disclosure, enabled state,
focus, and revision-bound action references. No caller can supply an
independent accessibility label, tree, state, focus, or action set. Projection
after commit is infallible; one side cannot succeed without the other.

Standard depth produces exactly 25 paint commands. Flat depth removes only the
two highlight and two shadow commands, producing 21. Focus and enabled state
use fixed command slots whose active state and token change without changing
the count. Both modes retain labels, region borders, control outlines,
enabled-state markers, and the explicit focus outline. Each projection always
contains eight accessibility records, eight machine-semantic records, and two
machine action descriptors. Different valid protected labels under otherwise
identical state change none of those counts, geometry, order, tokens, errors,
or observations.

Every paint command binds its canonical node identity and slot. A control
command also binds the exact revision-scoped action reference. The
accessibility and machine records independently carry the same tuple so the
cross-check covers visible controls rather than only comparing aggregate
counts.

Paint commands carry semantic color and shape tokens rather than pixels,
device formats, font metrics, locale state, callbacks, or Rite payloads. ADR
0024 remains the separate presentation mechanism beneath a future renderer.

The standard paint stream has this exact zero-based order:

1. `fill(root)`;
2. `fill`, `outline`, `label` for masthead;
3. `shadow`, `fill`, `highlight`, `outline`, `label` for navigation;
4. `fill`, `outline`, `label`, `enabled_marker`, `focus_outline` for primary;
5. the same five commands for secondary;
6. `shadow`, `fill`, `highlight`, `outline` for content;
7. `label(heading)`; and
8. `label(status)`.

Flat mode removes only standard indices 4, 6, 19, and 21: navigation and
content shadow/highlight. Every exposed field of each corresponding retained
command is identical.
Shadow precedes fill, highlight follows fill, and outline is the final shape
command for each depth surface; a later label does not alter its geometry or
depth. State and focus commands always occupy their fixed slots and carry an
explicit state-active boolean, so enabled/focused changes never alter the
stream length. That boolean is not a command-presence or renderer-skip flag: a
false enabled marker still renders its explicit disabled shape; a false focus
slot carries `focus_absent`. Focus is a shape-redundant double outline rather
than a color-only cue.

The fixed palette is:

| Token | RGB |
| --- | --- |
| canvas | `#E7ECF3` |
| surface | `#F5F7FB` |
| primary fill | `#1E40AF` |
| secondary fill | `#E2E8F0` |
| disabled | `#CBD5E1` |
| primary text | `#172033` |
| secondary text | `#405168` |
| on-accent text | `#FFFFFF` |
| outline | `#52657A` |
| focus outer | `#111827` |
| focus inner | `#FFFFFF` |
| public status | `#14532D` |
| protected status | `#6B21A8` |
| decorative shadow | `#A9B4C4` |
| decorative highlight | `#FFFFFF` |

The independent fixture oracle requires at least 4.5:1 for every actual text
pair: primary text against surface, secondary fill, and disabled fill;
secondary text against disabled fill; on-accent text against primary fill; and
public-status and protected-status text against surface. It requires at least
3:1 for each state marker: on-accent against primary fill, public-status
against secondary fill, and outline against disabled fill. A component
boundary passes when either its fill contrasts with its parent by at least 3:1
or its outline contrasts with both fill and parent by at least 3:1. The explicit
double focus ring places focus-inner beside the component outline, focus-outer
beside focus-inner, and focus-outer beside the parent surface; each adjacency
must reach 3:1. The deliberately decorative shadow and highlight are exempt
and forbidden from carrying state. These are scoped fixture checks, not
accessibility certification.

### Keep action and focus references non-authoritative

Every emitted action reference binds the exact public scene identity, committed
revision, node identity, action identity, and action kind. It is a selector,
not authority. Equal scalar values, labels, coordinates, focus positions,
pixels, or role tags never invoke anything.

Both committed projections may mint separate move-only local handles for an
exact action record. The handle binds the unexposed scene instance, committed
revision, node, action, and enabled-state tuple. `resolve_action` consumes it,
rejects a stale or foreign owner before inspecting current node state, rejects
a disabled action, and otherwise returns move-only semantic metadata only. A
rejection returns the complete handle. Resolution performs no effect. A future
invocation path must still revalidate the current scene and enabled state,
authenticate its principal, and pass sigil and policy checks.

The resolution-source tag records only whether the accessible or machine
projection minted the handle. It is not caller provenance, human presence, an
authenticated machine identity, a principal, or authority, and policy must
never branch on it as though it were one.

Both projections also mint instance- and revision-bound local focus handles.
Forward or reverse traversal consumes one valid handle and returns the next
enabled control handle with wrap; rejection returns the original. Projection
may mint the first handle when no control is currently focused. Traversal does
not mutate canonical focus: changing recorded focus requires a newly prepared
and committed scene revision.

Canonical focus traversal visits enabled controls in primary-then-secondary
preorder, wraps in both directions, and never sorts by geometry, label content,
hash order, or protected data. Machine action selection is explicit and does
not silently inherit the human focus target.

### Make label disclosure explicit

The proof has two move-only bounded UTF-8 text owners:

- `declared_public_text` is an intentional caller assertion that the label may
  enter both local projections. The crate does not verify that assertion or
  authorize logging, persistence, transmission, or screenshots.
- `protected_text` owns bytes that never enter either projection. Both sides
  receive only the same fixed typed redaction placeholder.

Both types accept at most 96 bytes, require nonempty UTF-8, and reject tab,
line and paragraph separators, the frozen control scalars, bidirectional
overrides, and the invisible-format set enumerated by ADR 0021's
`private_text` contract: `U+0000..U+0008`, `U+000B..U+001F`,
`U+007F..U+009F`, `U+00AD`, `U+034F`, `U+061C`, `U+180E`, `U+FEFF`,
`U+200B..U+200F`, `U+202A..U+202E`, and `U+2060..U+206F`. This one-line scene
policy additionally rejects `U+0009`, `U+000A`, `U+2028`, `U+2029`, and the
interlinear annotation controls `U+FFF9..U+FFFB`. This reduces spoofing in the
fixture but does not claim homoglyph resistance or locale-aware text security.

Only the nonactionable status slot may carry protected text. A protected
actionable or focusable node is rejected because neither a person nor a machine
could inspect the action label. Node existence, role, fixed geometry, and
disclosure class remain public in this proof; only protected label bytes and
their exact length are confidential.

Ordinary label text such as "verified", "trusted", "consent", or "system"
cannot create a privileged role, secure-attention surface, identity proof, or
policy result. Version 1 has no such role.

### Keep diagnostics content-free

Errors format only as static codes. The privacy observation contains
only a closed event and result plus `content_redacted=true` and
`history_persisted=false`. It omits labels, disclosure class, identities,
revisions, counts, geometry, lengths, excerpts, digests, and timestamps.
Neither safe formatting shape authorizes a telemetry sink or retention.
The public constructor makes event and result caller-selected shape data, not
an authentic operation receipt, audit record, or proof that an event occurred.
A future trusted observer must stamp observations at the operation boundary;
policy and audit cannot rely on this proof value's provenance.

Prepared definitions, rejections that return private ownership, scene owners,
projection owners, local action/focus handles, resolution owners, and
retirement owners remain move-only and non-formatting. Scalar identities,
roles, tokens, and revision-bound references may be copied for matching, but
no API accepts them alone as authority.

## Required invariants

- Exactly one privately branded live owner and control govern a scene
  incarnation; public numbers never confer authority.
- A committed revision is complete, immutable to readers, and nonwrapping.
- Preparation validates and derives both projections without mutating the live
  scene; commit publishes the canonical definition and projection pair once.
- The fixed topology, geometry, roles, identities, actions, state, disclosure,
  focus, and all output budgets agree before commit.
- Every node appears exactly once and in the same order in the accessibility
  and machine projections. Every action reference and focus fact is identical.
- Human state never depends only on color, highlight, shadow, or motion.
  Labels, borders, outlines, and state markers remain in flat and reduced
  modes.
- Protected label bytes, lengths, excerpts, and digests affect no projection
  content, geometry, order, count, error, observation, or action/focus path.
- Rejection is failure-atomic and returns private ownership. Retirement remains
  possible after revision exhaustion and clears owned storage.
- Definitions, candidates, prepared revisions, stale rejections, committed
  owners, projection owners, successful replacement, `Drop`, and retirement
  share one logical-clear path for owned text. Prepared projections remain
  inaccessible until successful commit.
- Scene focus cannot select a session route. Action and focus references cannot
  execute an effect or grant authority.

## Non-goals

This decision does not define or implement:

- a compositor, window server, window/surface protocol, z-order, occlusion,
  clipping, hit testing, damage, animation, or application surface;
- input routing, pointer/touch/IME handling, focus arbitration, secure
  attention, trusted chrome, consent, clipboard, drag and drop, or screen
  reading;
- action execution, invocation, preview/effect binding, authentication, sigil
  checks, policy, audit, cancellation, or compensation;
- general scene graphs, dynamic layout, fonts, shaping, localization, scaling,
  themes, user customization, multiple displays, or a widget toolkit;
- pixels, GPU commands, ADR 0024 transport integration, capture authority, or
  Eidolon integration;
- serialization, a wire format, stable ABI, persistence, concurrency, threads,
  SMP ordering, crash recovery, secure erasure, or side-channel resistance; or
- WCAG, platform accessibility, usability, or visual-design certification.

## Acceptance requirements

Host tests must cover:

- zero and exhausted generational identities and revisions; wrong-slot,
  wrong-role, wrong-parent, stale, and cross-scene inputs; equal public scene
  identities isolated by distinct private brands; and zero or duplicate
  node/action identities;
- empty, exact-limit, over-limit, malformed UTF-8, control, line-break, bidi,
  and invisible-format text plus complete logical clearing;
- the complete 96-candidate matrix with exactly 64 accepts and 32
  focus-disabled rejections;
- exact 25/21 paint-command, eight-node accessibility, eight-node machine, and
  two-action budgets against an independent fixture oracle;
- repeated deterministic projection, canonical focus traversal, exact
  human/accessibility/machine role-state-focus-action parity, and the exact
  permitted standard-versus-flat difference;
- protected labels of different lengths producing identical redaction,
  geometry, counts, order, errors, and observation shape, with protected action
  or focus injection rejected;
- wrong control, equal public identity with wrong private brand, competing and
  stale preparations, replay, maximum revision, retirement, and stale handles
  against a fresh owner;
- public action references failing to resolve without an instance-bound local
  handle, disabled actions refusing resolution, and machine-explicit selection
  remaining independent of human focus;
- fixed palette contrast for the fixture, while making no certification claim;
  and
- compile-fail ownership/privacy coverage for every sensitive owner: no
  cloning, formatting, or hashing; no implicit string-like dereference or
  conversion for content-bearing text and projection labels; and no public
  field construction surface for private authority, preparation, handle, or
  projection owners.

Acceptance also requires formatting, warning-denied Clippy, the complete
configured host suite, the established non-VirtualBox QEMU regression matrix,
and normal-image exclusion. VirtualBox is not required because this host-only
crate changes no boot, kernel, ABI, or image path.

## Acceptance evidence

Acceptance was established at implementation checkpoint `2857d8c`. The
dependency-free `anomalous-desktop-scene` crate is `no_std`, allocation-free,
unsafe-free, and absent from the kernel's normal dependency tree.

The complete configured host check passed with 18 scene unit tests and the
then-current 128 compile-fail documentation tests, alongside 22
`anomalous-abi` unit tests plus 6 documentation tests, 51 `anomalous-session`
unit tests plus 56 documentation tests, 24 `anomalous-session-router` unit
tests plus 46 documentation tests, 307 focused display-feature kernel tests,
and 46 `xtask` tests. The final symmetric raw-focus-reference boundary brought
the scene documentation suite to 129 passing compile-fail tests. Formatting,
script syntax, diff checks, and warning-denied host and x86-64 freestanding
Clippy passed.

The scene tests enumerate all 96 declared construction states, accept exactly
64, and reject exactly 32 invalid focus states. Separate malformed and invalid
replacement tests prove that preparation leaves the committed scene unchanged.
Independent oracles verify the 25-command standard and 21-command flat plans,
eight accessibility records, eight machine records, two action descriptors,
fixed geometry, token/shape semantics, contrast, protected-label
noninterference, revision lifecycle, and action/focus handle boundaries.
Independent architecture and security reviews reported no unresolved P0, P1,
or P2 finding.

Every established non-VirtualBox QEMU lane passed serially: `smoke`,
`fault-smoke`, `circle-smoke`, `control-smoke`, `rite-smoke`,
`scheduler-smoke`, `supervision-smoke`, `shell-vessel-smoke`, `memory-smoke`,
`interrupt-smoke`, `input-wait-smoke`, `shell-smoke`, and
`display-vessel-smoke`. `normal-exclusion` also passed. VirtualBox was not run
or required because the accepted crate changes no native image path.

## Consequences

Anomalous gains a falsifiable first form of its "one world model, multiple
projections" thesis. Human rendering, accessibility, and machine inspection can
be compared at the exact revision and action identity rather than reconciled
after pixels exist.

The proof is intentionally not reusable desktop infrastructure. Its exact
slots and geometry make complete state and output evidence tractable. A later
decision may generalize the model only after preserving atomic revision,
disclosure, projection-parity, action-reference, and resource-bound invariants.

The following native slice can place this accepted contract inside a
supervised user-space renderer and feed its render plan through ADR 0024's
bounded display path. Window lifecycle, input/focus policy, and action
invocation remain separate decisions.

## Alternatives considered

### Make the framebuffer or screenshot authoritative

Rejected. Pixels lose roles, relationships, action identity, disclosure, and
nonvisual state and force machine clients to scrape a human projection.

### Let the renderer author a separate accessibility tree

Rejected. Independently mutable visible and accessibility state can drift and
create controls that exist for only one client class.

### Expose a machine-only desktop object API

Rejected. It could create undiscoverable actions or state unavailable to the
person affected by them.

### Define a general compositor and window protocol now

Deferred. That would combine lifecycle, resource ownership, ordering, input,
policy, rendering, and failure recovery before the canonical-state invariant
has independent evidence.

### Put the semantic scene in the kernel

Rejected. Roles, labels, layout, accessibility, and interaction policy do not
require supervisor privilege and would add avoidable privileged complexity.
