# ADR 0026: Compose a fixed supervised semantic renderer

- Status: Accepted
- Date: 2026-09-19

This record accepts one removable x86-64 proof in which a generated,
feature-private rendering capsule from ADR 0025 is consumed by a separately
linked ring-3 renderer, staged through ADR 0024's bounded display operation,
and recovered once under ADR 0020's supervision model. The proof keeps the
canonical scene and its authority-bearing owners on the host side, gives the
renderer only `display_update | task_exit`, and publishes no framebuffer pixel
until a complete replacement generation has exited successfully. It does not
define a live scene service, compositor, window or surface protocol, stable
capsule or graphics ABI, input/action path, or normal-image desktop runtime.

## Context

ADR 0025 accepts one host-only, `no_std`, allocation-free, unsafe-free scene
crate. A single committed revision derives its accessible render plan,
accessibility tree, and machine projection together. The accepted boundary is
deliberately not linked into the kernel, a native image, or a vessel. ADR 0024
accepts the lower presentation mechanism: a narrowly granted ring-3 task can
submit copied and validated RGB8 patches while the framebuffer remains
supervisor-only. ADR 0020 accepts one persistent supervisor, one destructive
restart, and exact terminal wake across fresh task-bound Rite channels.

Those pieces do not yet prove that they compose. Copying scene construction,
private branding, protected source text, or machine/action handles into the
kernel would put nonprivileged desktop policy in ring 0. Giving a renderer a
separately authored picture would permit the visible result to drift from the
accepted semantic revision. Letting generation 1 write live framebuffer rows
before its expected fault would also leave a mixed old/new frame, even though
each individual ADR 0024 patch is failure-atomic.

The next slice must therefore preserve three boundaries at once:

- the host scene remains the authority for paired human and machine
  projections;
- the unprivileged renderer receives only immutable public drawing material
  and a bounded pixel-output right; and
- a renderer fault can discard a whole frame without exposing a partial
  semantic revision.

The proof remains intentionally fixed. It should establish the composition
and failure boundary before adding a dynamic scene transport, font service,
window lifecycle, input policy, or application surface.

## Decision

### Add a separate top-level feature proof

Add `supervised_renderer_test` as a separate, mutually exclusive kernel proof
rather than extending `display_vessel_test` or `shell_vessel_test` in place.
It composes the accepted scene, Rite, scheduler, supervision, memory, and
presentation mechanisms while preserving their existing standalone evidence.
The feature and all of its payloads, entry gates, helpers, markers, and test
policy remain absent from the ordinary image.

`renderer`, `render_capsule`, and related names are conventional descriptions
of this bounded experiment. This decision does not introduce a new occult
platform abstraction.

### Compose the scene at build time without moving its authority boundary

`anomalous-desktop-scene` remains host-only. A host build/conformance step
constructs the real ADR 0025 scene, borrows both projections, proves their
existing parity, and emits one immutable feature-private render capsule. The
target renderer does not link the scene crate and never receives a
`desktop_scene`, `scene_control`, private instance brand, protected source
owner, prepared revision, machine projection, local action handle, or local
focus handle.

The exact source fixture is committed revision 1 at 1024x768 with:

- eight nodes in ADR 0025's canonical order;
- standard neumorphic depth and reduced motion;
- the primary action enabled and focused;
- the secondary action disabled;
- protected status disclosure;
- exactly 25 semantic paint commands; and
- exactly two revision-bound actions in the paired host projection.

Before emitting anything, the host step independently requires the exact
eight-node accessible/machine equality and two-action order accepted by ADR
0025. It then projects only the public, renderer-required subset: command
order, command and shape tokens, semantic colors, canonical slot/node binding,
checked rectangles, active-state shape, and the already disclosed label or
fixed `Protected status` redaction. Action references, focus handles, private
brands, construction controls, and protected source bytes are omitted. The
focused shape remains visible because it is part of the accepted paint plan;
it is not an input route or focus authority.

The capsule is a generated build artifact with a fixed maximum of 4 KiB. It is
not a public serialization, package format, IPC body, stable ABI, or format
negotiation promise. Its private representation may change with this feature.
The target build includes it in the renderer's immutable image extent, and a
target-side bounded validator rejects any count, order, enum, rectangle,
label, or budget mismatch before rendering its first row. Build failure and
validation failure expose only static error kinds and never protected source
content.

This is build-time composition, not a deployed scene service. A later live
desktop must define revision transfer, freshness, ownership, backpressure,
policy, and failure behavior separately.

### Use a separately linked fixed-address ring-3 image

Build the renderer as its own fixed-address, freestanding x86-64 `no_std`
image. It uses a small allocation-free renderer library and the generated
capsule, then calls the existing x86-64 Rite gate. The kernel packages the
result as an opaque feature payload, copies it into allocator-owned user
frames, and enters only its inspected fixed entry point. Kernel Rust neither
links nor calls renderer Rust text, and renderer symbols do not resolve into
the kernel link.

The image has exactly three logical user extents:

1. one immutable image extent of at most 64 pages, mapped RX and containing
   text plus the read-only capsule and fixed glyph/raster tables;
2. one page-sized RW+NX Rite mailbox; and
3. one RW+NX stack extent of at most four pages.

Adjacent guard pages remain absent. The linked image has no dynamic loader,
dynamic linking, runtime relocation, TLS, writable executable segment, or
entry outside the immutable image extent. Host inspection requires x86-64 ELF,
fixed canonical user virtual addresses, a bounded file/memory image, no
relocation sections or records, no W+X page, no writable alias of immutable
bytes, and no user mapping of the framebuffer or kernel-owned stage. Exact
post-link page counts become verifier inputs beneath these caps rather than
caller claims.

The renderer receives one task-bound grant containing only
`display_update | task_exit`. It receives no input, console, scene, action,
mapping, allocation, capture, or framebuffer right. Numeric task, scene, node,
revision, sequence, or address values confer no authority.

### Rasterize one exact public frame deterministically

The renderer validates the capsule once and rasterizes its 25 commands in
their canonical order with checked integer arithmetic. Rasterization is
integer-only, allocation-free, time-independent, and row-local. It has one
fixed bitmap glyph subset and one exact feature-private mapping from the seven
ADR 0025 paint-command kinds to RGB8 pixels. There is no shaping, localization,
animation, interpolation, GPU path, floating point, ambient theme state, or
input-dependent branch.

Each accepted update uses ADR 0024's existing version-1 RGB8 patch body and
carries exactly one complete 1024-pixel row. The target-bound staging policy
requires `x = 0`, `width = 1024`, `height = 1`, `row_bytes = 3072`, canonical
RGB8, and `y` equal to the next missing row. Duplicate, skipped, reordered,
partial, oversized, malformed, or post-completion rows reject without changing
the stage or advancing its sequence. This stricter per-target policy does not
turn the provisional ADR 0024 patch body into a stable graphics ABI.

The host oracle constructs the same real scene through ADR 0025, rechecks the
accessible/machine parity, and rasterizes the documented feature rules through
an implementation independent of the target renderer. It compares capsule
fields and every one of the 786,432 final RGB pixels. Pixel equality is
presentation evidence; pixels remain non-authoritative for roles, focus,
actions, identity, consent, or machine control.

### Stage one complete frame per renderer generation

Bind each renderer incarnation to one move-only kernel-owned stage comprising
exactly 576 contiguous allocator frames:

`1024 * 768 * 3 = 2,359,296 = 576 * 4,096` bytes.

The stage is supervisor-only, zeroed and verified before use, and associated
with the exact task incarnation, Rite channel, expected row, and a fresh
nonwrapping stage generation. A copied and fully validated row commits to this
stage, never to the framebuffer. The task cannot read the stage, retain it
across retirement, or select another task's stage by supplying an address or
identity.

Completion requires exactly 768 accepted rows in ascending order. A terminal
exit before complete coverage is not a successful rendered-frame observation.
Fault, denial, malformed input, stale identity, early exit, or teardown clears
and verifies the complete stage before releasing its lease. Physical reuse is
permitted only after that withdrawal and clear; a fresh logical stage
generation never inherits completion or pixels.

Only one 576-page stage is live at a time. This proof allocates no second live
frame, damage history, rollback image, shared user buffer, or page-flip queue.

### Supervise one fault and one successful replacement

Keep one ring-3 supervisor alive across exactly two renderer incarnations.
Both incarnations consume the same immutable public capsule from separately
cleared task/address-space state and receive fresh task, circle, channel,
sequence, and stage identities under one persistent supervision relationship.

Generation 1 first survives exactly one controlled real PIT preemption. The
timer is then masked before it stages rows 0 through 383 and executes one exact
expected `ud2`. The kernel derives the fault observation from the active task
and exact vector, selector,
instruction address, and saved frame. The supervisor consumes that observation
and authorizes its one destructive restart. At the established safe
kernel-stack checkpoint, the kernel withdraws generation 1's task, Rite
channel, mappings, root, privilege stack, copied scratch, and complete stage,
verifies their clearing, and proves that every framebuffer byte still matches
the pre-proof baseline.

Generation 2 begins with fresh sequence one, stages rows 0 through 767, and
invokes terminal `task_exit`. Complete stage coverage is checked before the
exit can produce the successful renderer observation. The exit copies no
reply, never returns to the retired renderer, and selects the persistent
supervisor. The successful observation transfers only the complete prepared
stage owner; it does not transfer task authority or make renderer-supplied
identities trustworthy.

The resumed supervisor consumes the exact success observation and reaches a
safe kernel-stack checkpoint. With interrupts masked and the registered
interrupt-failure surface still present, it first retires the renderer,
supervisor, scheduler and supervision state, task and circle ownership,
mappings, Rite channel, and private task storage while retaining only the
successful unbound stage owner. It validates that pre-withdraw teardown and
the resulting retained-stage accounting exactly, then rechecks the complete
pre-proof framebuffer baseline. Only then does it consume the registered
interrupt-failure surface.

After withdrawal, from the safe kernel stack, the kernel completes the
remaining fallible publication preflight: it revalidates the exact successful
stage identity and completeness, constructs the unique presenter, proves the
framebuffer geometry and complete write extent, obtains complete-frame
publication evidence, and prepares and range-checks the allocator metadata
transition for the stage lease. Only after those choices are fixed does it
perform one non-rejecting full-frame copy from the prepared stage. No renderer
task, Rite handler, scheduler callback, registered framebuffer failure path,
or feature exception path remains able to interleave with that copy.

This is framebuffer-content failure-atomicity for the declared software
transitions. Every modeled rejection or generation-1 fault occurs before the
first live framebuffer write. After the first write, no fallible
publication-selection, source-completeness, geometry, ownership-metadata, or
copy-shaping decision remains; the already bounded copy runs to completion
under the proof's software model.

That scoped claim does not pretend that all later lifetime work is infallible.
After the complete frame has been copied, physical stage clear and readback
can still report failure. Such a failure does not reject, shorten, or roll back
the completed publication: the complete selected frame remains published, the
stage lease remains active and allocator-owned in quarantine, no physical
reuse is authorized, and the proof emits neither success nor exact-restoration
evidence before halting. If clear and readback succeed, metadata retirement is
non-rejecting, the lease is released, and exact allocator accounting is
restored.

The claim is not a hardware page flip, scanout atomicity, vsync, tear-free
presentation, power-loss atomicity, machine-check recovery, or proof against a
bug in the final copy. A display engine may scan different rows while the
non-rejecting copy is in progress. Successful proof completion additionally
requires retirement of all feature ownership and exact restoration of
scheduler, supervision, task, circle, mapping, Rite-channel, allocator, and
interrupt accounting.

### Keep the proof private by default and capture explicit

The build fixture's protected source owner exists only in the host conformance
step. Its bytes, length, excerpt, digest, allocation residue, and error text do
not enter the capsule, renderer image, mailbox, stage metadata, kernel,
structured serial records, screenshot, or interactive output. The capsule and
final frame contain only ADR 0025's fixed `Protected status` redaction. Two
different valid protected source values under otherwise identical fixture
state must generate byte-identical capsules and pixels.

Renderer-owned success evidence is closed and content-free. It may report only
fixed lifecycle kinds, generation, bounded row counts, exact resource counts,
and boolean invariant results. It omits label bytes, capsule bytes, pixel
samples, framebuffer or user addresses, trace values, caller-selected text,
and protected-source metadata. Copied Rite scratch, mailbox and stack pages,
stages, task images, and supervision state are cleared and verified before
reuse. This is logical lifetime evidence, not a secure-erasure or side-channel
claim.

The automated target may take one explicit strict-P6 capture only after the
successful final publication. That known public-redaction fixture remains
under ignored `build/` storage and is neither telemetry nor ambient recording.
A companion interactive command may display the same inspected feature image
for human review. Neither path grants screenshot authority to the renderer,
defines retention policy for real user content, or integrates Eidolon.

### Fix the proof budget

The complete native experiment has:

- one persistent supervisor and at most one live renderer;
- exactly two renderer incarnations and one restart authorization;
- exactly one real PIT preemption and one exact generation-1 `#UD`;
- 384 generation-1 and 768 generation-2 accepted display rows;
- one generation-2 terminal exit and no generation-1 exit;
- at most 64 immutable image pages, one mailbox page, four stack pages, and
  one 576-page stage at a time, in addition to bounded existing page-table and
  privilege-stack ownership; and
- no retry loop, concurrent display client, dynamic allocation in ring 3,
  unbounded label, variable command stream, or runtime capsule replacement.

Every counter is checked for exact completion or exact restoration. Reaching a
cap is not authority to exceed it.

## Required invariants

- The host scene owner remains the only canonical source; target code consumes
  a stripped immutable capsule generated only after exact human/accessibility/
  machine projection parity succeeds.
- Protected source bytes and authority-bearing scene/control/action/focus
  owners never cross into the target image or kernel.
- The separately linked renderer has fixed inspected mappings, no relocation,
  no W+X page, no framebuffer/stage mapping, and only
  `display_update | task_exit`.
- Every patch is copied once, validated completely, and bound to the current
  task/channel/stage generation before exactly one sequential row changes.
- A stage is complete only after rows 0 through 767 commit exactly once and in
  order. Stale, partial, failed, or retired stages can never publish.
- Generation 1 changes no framebuffer byte. Its expected fault is consumed by
  the persistent supervisor before destructive replacement at a safe kernel-
  stack checkpoint.
- Generation 2 terminally exits without return; only the supervisor can
  consume its successful complete-stage observation.
- Pre-withdraw teardown leaves only the successful unbound stage owner and
  proves exact retained-stage accounting. The interrupt-failure surface is
  then withdrawn under masked interrupts before the remaining publication
  preflight, unique presenter construction, and final copy.
- All fallible publication-selection, source-completeness, geometry,
  ownership-metadata, and copy-shaping decisions precede the first framebuffer
  write. Post-copy stage clear/readback failure preserves the complete
  published frame, keeps the lease owned and quarantined, authorizes no reuse,
  and prevents proof success. The resulting claim is software framebuffer-
  content failure-atomicity, not scanout-atomic or tear-free.
- The complete frame matches an independent oracle, while pixels and capsule
  fields confer no semantic or action authority.
- Successful teardown clears private mutable storage and restores every
  tracked resource. A failed verified clear retains allocator ownership in
  quarantine and cannot report restoration or success; the normal image
  contains no feature code, capsule, target payload, gate, marker, or helper.

## Non-goals

This decision does not define or implement:

- a live scene host, scene IPC, general renderer protocol, compositor, window
  server, window/surface lifecycle, z-order, occlusion, application surface,
  or production display manager;
- a stable capsule, scene, font, raster, surface, or graphics ABI, or a normal-
  image user-space runtime;
- dynamic revisions, arbitrary scenes, variable nodes or commands, layout,
  shaping, localization, scaling, themes, multiple displays, animation,
  damage, caching, or retained rendering;
- GPU commands, DMA, acceleration, shared buffers, zero-copy presentation,
  page flipping, fences, vsync, scanout atomicity, HDR, or color management;
- input routing, focus arbitration, action invocation, secure attention,
  consent, authentication, authorization, sigil policy, clipboard, drag and
  drop, screen reading, or application launch;
- a native machine-scene service or AI-control endpoint; the paired machine
  projection is checked by the host conformance step and is not transported to
  the renderer;
- general task launch, production scheduling or supervision policy, repeated
  crash recovery, concurrency, SMP ordering, or recoverable final-copy faults;
- ambient screenshots, telemetry, history, Oracle or Eidolon capture, crash
  artifacts containing content, persistence, secure erasure, or side-channel
  resistance; or
- WCAG, platform-accessibility, usability, visual-design, or security
  certification.

## Acceptance requirements

Host and build tests must cover:

- construction of the exact revision-1 ADR 0025 fixture and equality of all
  eight accessible/machine nodes, both actions, focus, state, disclosure,
  geometry, labels/redaction, and the 25-command standard plan before capsule
  emission;
- two otherwise identical target builds differing only in their valid
  protected source label, with byte-identical capsules and an exact
  byte-for-byte comparison of the complete target ELF files, plus identical
  final pixels, error shapes, and observation shapes, with neither source
  present in generated artifacts or logs;
- exact capsule size and budget plus rejection of wrong count/order, enum,
  shape, color, state, slot/node binding, geometry, label, UTF-8, truncation,
  trailing data, and viewport values;
- deterministic target rendering of every row, canary-protected bounds, exact
  paint order, glyph coverage, and an independently implemented complete-frame
  oracle for all 786,432 RGB pixels;
- stage ownership, strict row ordering, duplicates, gaps, malformed and
  out-of-bounds patches, stale/wrong task/channel/generation identity, early
  exit, completion, clear-before-reuse, exhaustion, and full rejection
  atomicity;
- fault and restart transitions, single-supervisor ownership, exact successful
  stage transfer, no publication before success, presenter uniqueness,
  interrupt-surface withdrawal, and no fallible publication-selection,
  source-completeness, geometry, ownership-metadata, or copy-shaping decision
  after the first framebuffer write;
- injected post-copy clear/readback failure proving that the complete selected
  frame remains published, the stage lease remains owned and quarantined, no
  reuse is authorized, and no success or exact-restoration evidence is emitted;
- ELF machine, fixed entry/address, segment bounds, page permissions, guard
  gaps, no relocation/dynamic/TLS surface, no W+X or writable immutable alias,
  exact dependency direction, and absence of the scene crate from target and
  kernel links; and
- ordinary-image exclusion of every feature gate, helper, capsule, renderer
  payload, glyph fingerprint, marker, private source sentinel, and dedicated
  operation path.

The QEMU proof must show the exact narrow grant and inspected user extents,
one controlled PIT preemption followed by timer masking, 384 staged rows, and
the exact generation-1 `#UD`,
complete destructive cleanup, and a byte-for-byte unchanged framebuffer. It
must then show fresh generation-2 ownership, 768 staged rows, terminal exit
without return, persistent-supervisor wake and success consumption, masked
withdrawal of the registered interrupt/failure surface, one prepared
full-frame publication, successful stage clear and release, complete teardown,
and exact resource restoration. The complete successful transcript must
contain no error or fatal record and no protected-source bytes anywhere.
Feature-owned `kernel.supervised_renderer_test.*` records must contain no
pixel, label, address, trace, capsule, protected-source, or other declared
private field.

The retained strict-P6 capture must be exactly 1024x768 RGB8 and match the
independent semantic-scene oracle at every pixel. The interactive command must
boot the same inspected payload rather than an alternate image.

Acceptance also requires formatting, warning-denied host and x86-64 Clippy,
the complete configured host suite, an explicit all-feature freestanding link,
the established non-VirtualBox QEMU regression matrix, script checks, and
`normal-exclusion`. VirtualBox is not required for this removable QEMU
composition proof.

## Acceptance evidence

Accepted on 2026-09-19 against reviewed implementation checkpoint `562d5a4`.
`make check` passed with 329 all-feature kernel tests, 56 `xtask` tests, 8
renderer-base tests, 9 scene-conformance tests, the dual protected-source ELF
comparison, the explicit all-feature freestanding link, normal-image exclusion,
script checks, formatting, and warning-denied Clippy. The full established
non-VirtualBox QEMU regression matrix also passed. VirtualBox was not used for
this slice.

The final bounded `supervised-renderer-smoke` run observed the exact 21-record
canonical lifecycle, freshly measured final allocator restoration to 65,099
free frames with no active ownership, and matched all 786,432 pixels against
the independent 1024x768 oracle. The retained mode-0600 P6 capture has SHA-256
`4ada99e89a6beb3966a093b493b03e49c81b32de64c609ae457c00a56b76302c`; the
mode-0600 structured transcript has SHA-256
`5c164acf88fe3092ff66cbb13cbac5d7a08bbb3ae681f4165466bd8eab061ab9`.
Both remain ignored local evidence under `build/`, and no partial capture file
remained.

Independent final runtime and tooling reviews found no remaining actionable
P0, P1, or P2 issue after post-publication evidence was bound to a fresh
allocator observation and retained transcripts were restricted to exact CRLF
framing, fixed field order, and canonical spellings.

## Consequences

Anomalous now has one falsifiable bridge from a canonical
human/machine scene revision to supervised unprivileged pixels without moving
scene authority or framebuffer ownership into the kernel. A renderer crash
is shown to discard its entire incomplete private stage before one bounded
restart, and only a successfully exited replacement can publish.

The build-time capsule deliberately avoids deciding the harder live-runtime
contract. It proves that the accepted semantic plan can drive a native
renderer, not how a future scene service streams revisions, arbitrates windows,
routes input, invokes actions, or serves machine clients. The 2.25 MiB
contiguous stage and row-by-row Rite transport are evidence-oriented costs, not
performance architecture.

The proof can be removed by disabling one feature and deleting its private
target, generator, renderer, verifier, and tests. ADRs 0020, 0024, and 0025
remain independently valid.

## Alternatives considered

### Link the canonical scene crate into the kernel

Rejected. Scene construction, labels, accessibility, focus, and action
metadata do not require supervisor privilege and would enlarge ring-0 policy
and parsing surface.

### Give the ring-3 renderer the live scene owner and protected source

Deferred. It would require a native scene host, revision/freshness transport,
private ownership, restart, and service-lifecycle contract. The stripped build-
time capsule proves less with less authority and keeps ADR 0025 honestly host-
only.

### Hand-author the target fixture beside the host scene

Rejected. Two definitions could drift while still producing plausible pixels.
The capsule must be emitted from the actual accepted projection only after
accessible/machine parity succeeds.

### Let generation 1 update the framebuffer directly

Rejected. ADR 0024 makes each patch failure-atomic, not an entire 768-row
frame. A fault after row 383 would expose a mixed semantic revision.

### Add hardware page flipping or two live framebuffers now

Deferred. It would conflate renderer/supervision composition with display
controller ownership, scanout synchronization, and device-specific memory.
Kernel staging gives the bounded software failure boundary without claiming
tear-free presentation.

### Extend the accepted display-vessel feature in place

Rejected. The earlier proof intentionally has no supervision or canonical
scene. A separate top-level feature preserves its closed evidence and makes
normal-image and all-feature exclusion explicit.

### Define a compositor, window protocol, and input/action path together

Deferred. That would combine too many authority, lifecycle, privacy, and
resource questions before the fixed semantic-to-pixel restart boundary has
evidence.
