# ADR 0029: Prove bounded native semantic render-and-present composition

- Status: Accepted
- Date: 2026-09-20

Anomalous accepts one removable x86-64 proof that carries the exact redacted
candidate retained by ADR 0027 through ADR 0028's copied native transport,
renders it in a separately linked ring-3 consumer, presents one complete frame
through the accepted kernel staging path, and only then lets the same trusted
host coordinator publish the retained semantic transaction.

The proof covers one successful renderer attempt. Native retry, a deployed
desktop service, cross-process presentation authority, scanout completion, and
a normal-image desktop runtime remain outside this decision.

## Context

ADR 0025 accepts one canonical host scene whose human render plan,
accessibility tree, and machine projection share one revision. ADR 0026 proves
that a fixed build-time capsule can drive a supervised ring-3 renderer and that
a faulted partial frame can be discarded before one complete replacement frame
is copied to the framebuffer. ADR 0027 accepts one host-only transaction that
keeps revision N+1 private until an exact trusted presentation assertion lets
the paired semantic projections publish together. ADR 0028 accepts a copied,
versioned 528-byte native snapshot transport, but deliberately ends at
consumer delivery.

Those proofs leave a real composition gap. Delivery is not rendering. A
complete private stage is not presentation. A screenshot, transcript, digest,
surface number, renderer number, or receipt-shaped byte string is not ADR
0027 authority. Conversely, calling the host service's trusted presentation
operation before native pixels have been independently verified would merely
rename the missing evidence.

The next proof must preserve the reasons those boundaries were separated:

- the canonical scene and its privately branded controls remain host-only;
- the native renderer receives only redacted immutable drawing data and narrow
  output authority;
- the kernel owns transport, staging, task, supervisor, and framebuffer
  authority without acquiring scene, action, focus, or publication policy;
- no semantic revision advances because bytes were delivered, decoded, or
  partly painted; and
- the exact host transaction advances only after the exact native run and
  final pixels have passed closed-world verification.

ADR 0028's accepted fixture uses scene-state ordinal 60. ADR 0026's private
capsule uses ordinal 61. They differ in motion state even though the static
pixels are otherwise the same. This proof must not silently splice those two
artifacts together. It uses its own exact ordinal-61 transport body generated
from the live ADR 0027 transaction retained by its coordinator.

## Decision

### Add a separate highest-precedence feature proof

Add `semantic_surface_presentation_test` as a separate terminal feature with
explicit precedence over `semantic_surface_transport_test`,
`supervised_renderer_test`, and older terminal fixtures in combined-feature
builds. Its source, renderer, entry gates, helpers, stages, evidence, serial
markers, and generated artifacts remain absent from the ordinary image.

The proof is native x86-64. IA-32 execution and Win32 compatibility remain
future user-space veil work and cannot select or reinterpret this boundary.

`semantic surface`, `renderer`, `presentation`, and `coordinator` are
conventional descriptions of the experiment. This ADR introduces no new
occult platform abstraction.

### Retain one actual ADR 0027 transaction on the host

The trusted host coordinator constructs the real fixed ADR 0027 service at
presented revision 1, submits the ordinal-61 successor, acquires attempt one's
exact `render_lease`, and retains all privately branded owners needed to
complete or retire that transaction. It encodes the exact borrowed candidate
and matching lease through ADR 0028's named service adapter.

The coordinator owns this live transaction for the complete build, launch,
capture, and verification run. The resulting 528 bytes must be exactly the
body packaged into the proof source. Reconstructing an equivalent service from
public scalar fields after QEMU exits does not satisfy this contract.

The host service must continue exposing only revision 1 throughout source
submission, native delivery, rendering, staging, renderer exit, kernel
presentation, transcript verification, and pixel verification. Producer
capacity remains occupied throughout that interval.

The coordinator's private success path is:

1. consume the retained lease through `complete_render`;
2. use the retained presentation control through `attest_presented`;
3. consume the resulting receipt through `publish` with the retained surface
   control; and
4. require presented revision 2 and publication sequence 2 before retirement.

That path may execute only after a private, move-only verifier result for the
exact child process, kernel image, packaged body, transcript, and captured
frame exists. The verifier result is never serialized, logged, accepted from a
caller, or reconstructed from a digest.

Native output is verified evidence used by the existing trusted holder of ADR
0027 controls; it never reconstructs, transfers, or mints ADR 0027 authority.
This is a feature-private conformance composition, not a deployed cross-process
attestation protocol.

### Use one source and one combined transport renderer

The source remains a separately linked fixed-address ring-3 image. It receives
only `semantic_surface_submit | task_exit`, submits the exact generated body to
one kernel-owned endpoint, exits, and is completely retired before consumer
delivery. The endpoint's independent copied body remains live.

Add a distinct separately linked ring-3 renderer-consumer. Its exact grant is:

`semantic_surface_read | display_update | task_exit = 0x94`.

It receives no submit, console, physical input, semantic input, mapping,
allocation, capture, framebuffer, scene, action, focus, presentation, or
publication authority. The framebuffer and full-frame stage are never mapped
into ring 3.

The renderer reads the 528-byte body through the accepted header-last delivery
path, copies it into private owned target storage before reusing its Rite
mailbox, and independently validates the complete ordinal-61 profile. It then
rasterizes directly from the typed transport snapshot. It must not embed,
receive, reconstruct as bytes, or promote ADR 0026's private 416-byte capsule.

Mapping between transport command, shape, color, slot, label, and state tags is
performed through exhaustive named matches. Rust discriminant casts,
`transmute`, object-memory copying, padding assumptions, and unknown-tag
fallbacks are forbidden. The renderer validates all commands and labels before
submitting row zero.

The image retains the existing fixed-address target constraints: one bounded
RX image, one RW+NX Rite mailbox, one bounded RW+NX stack, absent adjacent
guards, no dynamic loader, relocations, TLS, W+X page, writable image alias,
floating-point/SIMD dependency, entropy, time source, or privileged instruction.

### Bind delivery, rendering, and terminal acknowledgement

The kernel endpoint retains one move-only successful delivery owner rather
than treating an empty slot as renderer completion. The proof binds that owner
to the exact source and renderer task incarnations, endpoint generation,
surface and scene identities, base and successor revisions, renderer
generation, attempt, stage generation, Rite channel, and supervisor child.

Each accepted `display_update` is one complete 1024-pixel RGB8 row and is
staged in strict ascending order into one zeroed, supervisor-only 576-frame
allocation. Delivery, validation, zero rows, a partial stage, a malformed or
reordered row, and an early exit produce no acknowledgement and cannot change
the live framebuffer.

The renderer's terminal `task_exit` becomes a render acknowledgement only when
all of the following are simultaneously true:

- the exact snapshot was delivered once and fully consumed;
- the exact renderer task and task-bound Rite channel remain current;
- all 768 rows and 2,359,296 RGB8 bytes occupy the exact bound stage;
- the successful-exit observation belongs to that renderer incarnation; and
- no stale, foreign, replayed, or caller-selected identity supplied authority.

The persistent supervisor consumes that exact successful observation. Complete
renderer, source, endpoint, scheduler, supervision, address-space, channel,
mailbox, stack, image, copied-scratch, and private task state is withdrawn and
cleared while only the unbound, exclusive completed-stage owner is retained.

The architecture-neutral supervision table therefore gains one distinct
completion-only lifecycle for an already active incarnation. It preserves the
same exact supervisor/incarnation identity, task-state, prepared-transition,
event-sequence, observation-consumption, withdrawal, and clearing checks, but
accepts only a successful exit and never spends or simulates a restart. The
existing fault-and-replacement lifecycle remains unchanged. This internal
kernel lifecycle is not a stable supervision ABI or caller-selectable retry
policy.

This slice proves attempt one only. If the renderer faults, exits early, or
otherwise fails, the run fails without framebuffer or semantic publication.
ADR 0026 continues to prove one bounded renderer replacement independently.
Native retry would require ADR 0027 to advance to a fresh renderer generation
and attempt two, followed by a newly encoded body, endpoint, task, channel,
stage, and supervision binding. Replaying or editing attempt-one bytes is not a
retry and is forbidden.

### Present only after complete teardown and preflight

Before native work, the proof establishes an exact declared framebuffer
baseline. That full-aperture pattern is a mutation oracle, not a claimed visual
rendering of ADR 0027 revision 1. Every modeled rejection and all partial
rendering must preserve every framebuffer byte while the host transaction and
all of its semantic projections remain at revision 1. Proving an initially
pixel/semantic-aligned revision-1 frame would require separately rendering and
verifying that projection and remains outside this slice.

After successful terminal acknowledgement, the kernel follows ADR 0026's
publication ordering. From a safe kernel stack with interrupts masked, it
withdraws all fallible feature state except the completed stage, unregisters
the diagnostic interrupt-failure surface, validates framebuffer geometry,
constructs the unique presenter, proves complete source coverage, and prepares
the allocator metadata transition. Only after all fallible choices are fixed
does it perform one non-rejecting complete-frame copy.

The kernel then reads back every visible framebuffer pixel and padding byte
against the exact completed stage and declared baseline padding, clears and
verifies the complete stage, releases its lease, and restores exact resource
accounting. It does not duplicate semantic rendering policy in ring 0. The
host runner obtains one private P6 capture only after the proof reports
success. An implementation-independent host oracle must compare all 786,432
RGB pixels before the coordinator may advance ADR 0027.

This establishes framebuffer-content evidence under the declared software
model. It is not a hardware page flip, scanout-completion proof, vsync,
tear-free presentation, DMA/GPU fence, power-loss atomicity, machine-check
recovery, or proof against a bug in the final bounded copy.

### Make the crash gap explicit

This composition does not provide cross-process or crash atomicity between
framebuffer mutation and semantic publication. If the host coordinator fails
after the complete native frame is copied but before ADR 0027 publishes, the
visible framebuffer may contain revision 2 while host semantic readers still
observe revision 1.

The proof runner treats that interval as fail-stop and produces no accepted
result, but it cannot roll back an already copied framebuffer. Closing the gap
requires a deployed native semantic service and a two-phase presentation
protocol, or a hardware/page-flip-style commit boundary, under a later ADR.

### Keep content private and evidence closed

Protected source bytes and exact lengths remain confined to the host scene
transaction. They do not enter the transport body, target images, kernel,
endpoint metadata, stage metadata, pixels, transcript, capture, diagnostics,
or verifier errors. Two unequal valid protected sources must produce
byte-identical transport bodies, source payloads, renderer results, final
pixels, transcripts, and success/error shapes.

Generated bodies and captures remain ignored build artifacts. Partial serial
and P6 files use owner-only permissions, are never echoed before validation,
and are deleted on rejection. Successful structured records contain only
fixed lifecycle kinds, bounded counts, resource restoration, and boolean
invariants. They omit snapshot bytes, labels, pixels, addresses, pointers,
surface/scene/renderer/task/endpoint values, revisions, attempts, Rite traces,
protected metadata, and timestamps.

Mutable source, renderer, mailbox, endpoint, stage, copied-scratch, task, and
supervision storage is cleared and verified before successful physical reuse.
This is logical lifetime evidence, not secure erasure, cache clearing,
activity/timing noninterference, or side-channel resistance.

## Required invariants

- The exact live ADR 0027 service transaction and lease survive from snapshot
  generation through native verification; public scalars cannot recreate them.
- The source, renderer, kernel, and host verifier each receive only the
  authority required for their role.
- The renderer consumes the exact transported body directly and never embeds
  or promotes ADR 0026's private capsule.
- Delivery and endpoint consumption do not complete the render lease or release
  producer backpressure.
- A render acknowledgement requires exact delivery, task, channel, supervisor,
  stage, binding, complete-row, and terminal-exit agreement.
- Completion-only supervision cannot be reached through a fault observation,
  replacement admission, stale identity, or caller-selected outcome.
- No modeled failure before the final framebuffer copy changes the declared
  baseline pixels or revision-1 semantics.
- The kernel presents only after complete feature teardown and complete
  preflight, then verifies all final framebuffer pixels.
- Only the host verifier that retains the exact private ADR 0027 owners can use
  successful native evidence to publish, exactly once.
- Human, accessibility, and machine projections remain at revision 1 until the
  final host commit and become revision 2 together.
- Protected inputs affect no native body, image, result, pixels, transcript, or
  capture, while operation activity and externally observed timing remain
  explicit nonclaims.
- All feature ownership and accounting are restored, and all feature code and
  data remain absent from the normal image.

## Non-goals

This decision does not define or implement:

- a native ADR 0027 service, portable presentation token, general
  acknowledgement ABI, remote attestation, or cross-process capability;
- renderer replacement, attempt-two delivery, retry, cancellation, recovery,
  or more than one source, renderer, endpoint, stage, or presentation;
- cross-process, power-loss, or crash atomicity between pixels and semantics;
- a proof that the declared initial framebuffer baseline visually represents
  the host transaction's revision-1 semantic projection;
- a stable semantic, scene, window, renderer, graphics, Rite, or package ABI;
- a compositor, window server, display manager, application surface, z-order,
  occlusion, damage, multiple displays, scaling, or normal-image desktop;
- input/focus routing, hit testing, action execution, secure attention,
  consent, authentication, clipboard, drag and drop, or application launch;
- a deployed accessibility or machine-control service;
- arbitrary scenes, command counts, labels, text, fonts, shaping,
  localization, animation, themes, or dynamic profiles;
- a GPU path, DMA, IOMMU proof, scanout completion, page flip, fence, vsync, or
  tear-free guarantee;
- SMP, threads, concurrent surface mutation, NMI enablement, or a general
  recoverable user-copy mechanism;
- ambient capture, telemetry, history, persistence, secure erasure, or
  side-channel resistance; or
- an IA-32 kernel or native foreign-platform ABI.

## Acceptance requirements

Host transaction and renderer tests must cover:

- exact ordinal-61 live-service construction, lease/view encoding, and equality
  with the body packaged into the native source;
- revision 1, publication sequence 1, paired projections, and occupied producer
  capacity remaining unchanged before verified native success;
- exact once-only `complete_render`, `attest_presented`, and `publish`
  progression to revision 2 and publication sequence 2;
- stale, foreign, equal-public-ID/different-brand, wrong-renderer,
  wrong-attempt, wrong-body, incomplete-transcript, and altered-frame evidence
  producing no publication;
- exhaustive named mapping of all accepted transport tags, both 21- and
  25-command forms, all 64 valid scene states, and rejection of invalid state
  or command combinations before row zero;
- direct transport-backed rasterization against an independent full-frame
  oracle; and
- two unequal protected sources producing identical allowed artifacts and no
  protected bytes in recursive artifact or diagnostic scans.

The x86-64 proof must show:

- separately inspected fixed-address source and renderer images, exact RX and
  RW+NX mappings, absent guards, disjoint narrow grants, and no user framebuffer
  or stage mapping;
- one complete submit, source retirement before delivery, header-last delivery,
  independent renderer validation, and endpoint consumption exactly once;
- exactly 768 ascending row commits into one 576-frame stage while the live
  framebuffer remains the exact declared pre-presentation baseline;
- one exact renderer terminal-success observation bound to delivery and stage,
  with early exit and partial completion rejected;
- teardown of all source, renderer, supervisor, scheduler, channel, endpoint,
  mapping, address-space, scratch, and private task state before presentation;
- one complete-frame preflight and copy, complete stage-to-framebuffer
  readback, stage clearing, allocator release, and exact accounting
  restoration; and
- a strict closed-world transcript and private P6 capture whose 786,432 pixels
  match an independent ordinal-61 semantic oracle.

Tooling acceptance requires formatting, warning-denied host and x86-64 Clippy,
target dependency and ELF inspection, wrapper and linked-payload inspection,
mutation tests for the transcript and private verifier gate, explicit
all-feature precedence and final linking, the complete configured non-
VirtualBox QEMU regression matrix, a dedicated semantic-presentation smoke
target, and final normal-image exclusion.

VirtualBox is not required because this proof does not change Threshold or the
ordinary-image boot path.

## Acceptance evidence

Accepted from implementation checkpoint `2f453e7` (`Implement native semantic
presentation composition`) plus acceptance-evidence hardening checkpoint
`e605f54` (`Harden semantic presentation acceptance evidence`). The retained
host transaction produced one exact 528-byte ordinal-61 snapshot with SHA-256
`831773c709ce0a651a7e3b4d4e9549465f7864f284356d074d7bb2d30041cb54`.
The separately inspected native artifacts were:

- source ELF: 1,107,792 bytes,
  `a2bcd57a22801fe821ddb089d0ecfefa2efd137facf28756878672aa1ef0c9a9`;
- source flat image: 2,384 bytes (one page),
  `8bab87d92fa74a43c47321c13743bb371c176169b83cf32bcb9f089a2723f645`;
- source wrapper: 3,848 bytes,
  `3ac25fc3177f11254ab037584a135be57a139fa6734842042eb6d35a09063bad`;
- renderer ELF: 1,239,056 bytes,
  `fcc9decbaddb8122063ac88a52c7dfd1da26b27b287fd4b926b1d4b491bb33ac`;
- renderer flat image: 10,352 bytes (three pages),
  `666c638bd52d76b4883ae0b4bb2a4892fb02fcba0e572440db49ab950e4edc74`; and
- renderer wrapper: 11,744 bytes,
  `74693a532c34d19141e6ebf0d0d240b04d98a5c5117495504ae534f6e25a5573`.

The dedicated successful QEMU run emitted exactly seven feature records:
`ready`, `source_released`, `render_acknowledged`, `prepared`, `presented`,
`released`, and `passed`. Its final counters were 777 Rite calls, 775 body
copies, 2,392,160 copied body bytes, 768 ordered rows, one submission, one
delivery, one backpressure result, two authority denials, one malformed
result, one freshness denial, two task exits, one successful-exit observation,
and one framebuffer commit. Teardown restored all 65,095 initial free frames
with zero owned data frames, table frames, or active leases.

The retained strict-P6 capture was 2,359,312 bytes with SHA-256
`4ada99e89a6beb3966a093b493b03e49c81b32de64c609ae457c00a56b76302c`;
the 2,572-byte closed-world serial log had SHA-256
`a19dcc7b1e5f43e99a07f70f863c9afa852f659393a43de5028dd52c6b5f0e6c`.
The kernel compared the completed stage with every visible framebuffer channel
byte and preserved declared baseline padding. The independent host oracle then
matched all 786,432 RGB pixels before consuming the private verifier result.
Only afterward did the retained ADR 0027 owners complete, attest, and publish
exactly once, advancing the presented revision and publication sequence from
1/1 to 2/2 together.

The native QEMU evidence exercises the successful renderer path only. Early
exit, partial-stage completion, stale or foreign identity, wrong outcome, and
wrong supervision phase are rejected by structural completion predicates and
architecture-neutral component tests; acceptance does not claim a separate
negative x86-64 early-exit run. Likewise, the pre-presentation framebuffer is
only the declared mutation baseline. It was proven unchanged before the final
copy, but was not claimed or tested as the visual rendering of ADR 0027
revision 1.

Acceptance verification included formatting, warning-denied host and x86-64
Clippy, target dependency and ELF inspection, wrapper and final-payload
inspection, transcript and verifier mutation tests, all-feature precedence and
strict final linking, the complete configured QEMU regression matrix, the
dedicated display-vessel, supervised-renderer, semantic-surface-transport, and
semantic-presentation smoke targets, and final normal-image exclusion. No
VirtualBox run was needed because neither Threshold nor the ordinary-image
boot path changed.

This evidence does not close the framebuffer-versus-host-publication crash
gap, prove scanout, provide native retry, establish a stable ABI, or introduce
a general compositor, window service, or normal-image desktop runtime.

## Consequences

With acceptance, Anomalous has one measured end-to-end proof that the same
host-retained semantic successor can cross the native byte boundary, drive an
isolated renderer, reach the framebuffer through a complete private stage, and
gate atomic publication of its coherent human, accessibility, and machine
semantics.

The result deliberately remains a conformance bridge. It exposes the next
architectural work rather than hiding it: a real desktop service must carry
presentation acknowledgement across processes without turning public bytes
into authority, and it must close the pixels-versus-semantics crash gap.

## Alternatives considered

### Treat successful ADR 0028 delivery as renderer completion

Rejected. Delivery proves only copied transport ownership. It says nothing
about rows, pixels, a completed stage, terminal renderer state, or framebuffer
presentation.

### Reuse ADR 0026's generated capsule

Rejected. The capsule is a feature-private build artifact for ordinal 61, not
a live transport, while ADR 0028's accepted fixture is ordinal 60. Promoting it
would both bypass the explicit transport contract and broaden a deliberate
nonclaim.

### Serialize a presentation receipt into the guest or transcript

Rejected. ADR 0027's receipt is authority because it is privately branded and
move-only. Public bytes, logs, hashes, screenshots, and matching numeric fields
are evidence or correlation only and cannot recreate it.

### Link the scene service into the kernel

Rejected. Scene construction, protected source ownership, paired semantic
projections, and publication policy do not belong in ring 0. The host verifier
can retain the actual transaction without expanding the kernel's policy TCB.

### Include the native retry in this slice

Deferred. A real retry requires the retained host service to authorize a fresh
renderer generation and attempt two, then generate and transport newly bound
bytes through fresh native ownership. Editing or replaying attempt-one bytes
would be a freshness violation rather than recovery.

### Claim atomic pixels and semantics across host failure

Rejected. The framebuffer copy and host semantic commit are distinct
operations in this proof. The failure interval is recorded explicitly until a
deployed two-phase or page-flip-style design can close it.
