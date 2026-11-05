# ADR 0024: Prove bounded user-space display presentation

- Status: Accepted
- Date: 2026-09-19

This record accepts one removable x86-64 proof in which a fixed ring-3
display vessel submits versioned, bounded RGB8 patches through the existing
`display_update` rite and a kernel-owned presenter converts them into the
validated boot framebuffer. The proof renders one declared nonsecret,
desktop-shaped fixture that can be captured automatically and opened
interactively for human review. It does not define a compositor, window
protocol, semantic desktop tree, production display manager, stable ABI, or
normal-image user-space runtime.

## Context

Before this decision, Threshold already transferred one validated packed-pixel
framebuffer to the kernel. The kernel owned that aperture exclusively and used
a private canvas for boot diagnostics and the bootstrap shell. Ring-3 tasks did
not receive its address or a user-accessible mapping. ADR 0019 reserved a
`display_update` right, operation, and payload class, but the kernel admitted
any nonempty payload in that class and had no versioned body, handler, or
user-space display proof.

The first visible user-space step must not turn a framebuffer address, a
caller-provided rights mask, or an unvalidated rectangle into authority. It
must also avoid treating pixels as the machine-facing desktop model. A future
compositor should own one canonical semantic scene and derive both accessible
human presentation and machine-readable state from it; this lower boundary
should remain a deliberately narrow presentation mechanism beneath that scene.

Mapping the boot framebuffer into a client would expose unrelated pixels,
firmware geometry, and device layout while allowing an uncontained writer to
overwrite secure or diagnostic surfaces. Interpreting a rich drawing language
in the kernel would add avoidable privileged attack surface. Committing from a
live user mapping would permit mutation during validation. Logging payloads,
trace identifiers, pixels, geometry, or screenshots by default would create a
privacy leak before a desktop privacy policy exists.

## Decision

### Add a versioned RGB8 patch body

Add `rite_display_patch_v1`, a fixed 40-byte little-endian header followed by
canonical tightly packed RGB8 pixels. Its exact fields are:

- `u16 structure_size` with value 40;
- `u16 version` with value 1;
- zero `u32 flags`;
- `u32 x`, `y`, `width`, and `height`;
- `u32 row_bytes`, equal to `width * 3`;
- `u16 pixel_format`, with version-1 value `rgb8 = 1`;
- zero `u16 reserved`;
- `u32 pixel_byte_length`, equal to `row_bytes * height`; and
- zero `u32 reserved_2`.

Width and height are nonzero. Every multiplication, addition, conversion, and
rectangle endpoint is checked. The outer Rite payload size must equal the
40-byte header plus `pixel_byte_length`, and its caller-declared payload limit
must equal that exact size. The existing one-page mailbox therefore limits one
patch to at most 4,016 pixel bytes after the header. Version 1 has no padding,
compression, alpha, palette, commands, shared-memory reference, or implicit
row stride.

Unknown versions, formats, flags, reserved values, inconsistent lengths,
wrapped geometry, empty rectangles, out-of-output rectangles, and any payload
that crosses the mailbox or kernel cap are rejected before presentation.
Unknown integer tags remain ordinary data; arbitrary bytes are never decoded
directly into a Rust enum discriminant.

This is an experimental pre-alpha body, not a stable application or driver
ABI. RGB8 is the canonical boundary format for this proof, not a promise about
future color management or GPU surfaces.

### Retain exclusive framebuffer ownership in the kernel

Add a move-only kernel presenter over the already validated framebuffer
description. Construction receives the unique aperture ownership currently
transferred by `kernel_main`; no second live writer is created. The presenter
retains the physical address, pitch, bits per pixel, and channel masks. None of
those values or mappings enter the ring-3 task.

The client keeps exactly the existing three user mappings: one RX code page,
one RW+NX Rite mailbox, and one RW+NX stack page. The borrowed supervisor
address-space branch remains supervisor-only. The boot framebuffer remains
reserved from the frame allocator.

After a successful Rite admission, architecture code copies the complete body
into the fixed kernel scratch page and stops consulting user memory. It decodes
and validates the header and pixel extent against the presenter's actual
output. Every fallible check occurs before the first framebuffer write. The
commit then converts canonical RGB8 through the trusted channel masks and
writes exactly the accepted rectangle. Under the proof's exclusive mapping
contract, that commit is infallible; hardware failure is fail-stop.

A successful Rite reply is created only after the complete patch commit.
Malformed, denied, stale, out-of-bounds, or failed-copy calls leave every
framebuffer byte unchanged. Scratch containing copied pixels is logically
cleared before reuse and teardown. This is disciplined lifetime handling, not
a secure-erasure claim for caches, registers, speculation, or hardware.

### Prove one fixed ring-3 display vessel

Add a `display_vessel_test` feature separate from the accepted `rite_test` and
`shell_vessel_test` features. It reuses their fixed x86-64 address-space and
Rite transport mechanisms but has its own closed-world task, gate lifecycle,
transcript, and normal-image exclusion evidence.

The task receives only `display_update | task_exit`. In particular it receives
neither console output, raw physical input, semantic session input, task yield,
nor ambient framebuffer authority. It submits a fixed sequence of valid
patches that covers one declared 1024x768 public desktop-like fixture, observes
the replies, and exits through the Rite gate. The fixture uses the project's
accessible neumorphic direction for visual review, including explicit border
and focus geometry in addition to light and shadow. Its content is nonsecret
and contains no user, credential, application, model, or session data.

The proof also exercises denied authority, malformed body, and out-of-bounds
rejection before the valid presentation, and proves those attempts leave the
complete canary-backed framebuffer aperture unchanged. Display-vessel-owned
success records contain only bounded lifecycle counts and public fixture
dimensions. They never emit pixel bytes, mailbox bytes, trace identifiers,
addresses, caller-selected labels, or sampled content. This does not redefine
the existing public boot telemetry for memory and display geometry. The smoke
verifier accepts that bounded platform evidence but rejects every error or
fatal record; fault-only developer diagnostics are outside the successful
transcript privacy claim.

The automated smoke target boots without a debug-exit device, waits until the
guest has completed the ring-3 lifecycle and teardown, captures the retained
public frame with QEMU `screendump`, asks QEMU to quit, strictly parses the P6
file, and compares every RGB8 pixel with an independent fixture oracle. The
artifact remains under ignored `build/`. A companion interactive command boots
the same proved image in a normal QEMU window and leaves the completed frame
visible for end-user review.

Eidolon is not linked into the kernel or required by this acceptance path.
Future explicit capture workflows may feed a user-selected public frame into
Eidolon under ADR 0017's limits and privacy rules.

### Keep pixels subordinate to semantic state

This rite proves presentation only. A pixel, color, shadow, rectangle, trace,
sequence, or screenshot confers no focus, input, action, identity, consent, or
control authority. Machine clients must not scrape the framebuffer to discover
desktop state, and the kernel does not accept a caller-asserted accessibility
tree alongside unrelated pixels.

A later decision must define a bounded user-space desktop scene whose single
committed revision derives both an accessible render plan and a semantic
machine projection. That layer must make keyboard traversal, focus, action
sets, labels, state, reduced motion, depth-disabled legibility, privacy, and
human/machine equivalence first-class. The present patch boundary can carry
that compositor's final pixels without becoming its semantic protocol.

## Required invariants

- Only an exact live task incarnation with a trusted `display_update` grant
  may reach patch decoding or framebuffer mutation.
- Caller-declared rights, numeric identities, sequences, traces, addresses,
  geometry, and pixel contents never confer authority.
- Ring 3 has no user-accessible framebuffer mapping and learns no physical
  address, pitch, or channel-mask layout through this operation. The proof
  walks the task root's borrowed upper entry and requires it to remain
  supervisor-only across the complete identity-mapped aperture.
- A complete body is copied into bounded kernel storage before decoding; user
  memory is never consulted during validation or commit.
- Header, wire length, row length, pixel length, rectangle endpoints, output
  bounds, mailbox span, and operation cap agree exactly before mutation.
- Every rejected operation is framebuffer-failure-atomic. Every accepted
  operation writes exactly one fully validated rectangle and only then replies.
- Unique presenter ownership prevents concurrent framebuffer writers during
  the proof. Retirement clears copied pixel scratch and invalidates the exact
  task, channel, mappings, and address space before their storage is reused.
- Display-owned success evidence is structured, content-free, and free of
  pixels, private transport data, traces, addresses, and screenshots; an
  accepted run otherwise contains only the existing bounded boot telemetry.
- The normal image contains no display-test gate, fixture, helper, private
  marker, or feature-only user payload.
- Human-visible pixels are presentation evidence only; authoritative semantic
  state and machine control remain outside this ABI.

## Non-goals

This decision does not define or implement:

- a compositor, window server, desktop shell, general display manager, stable
  surface protocol, or normal-image user-space runtime;
- a semantic scene, accessibility tree or service, focus policy, hit testing,
  z-order, window lifecycle, keyboard traversal, secure attention, or consent
  surface;
- input routing, pointer/touch/IME support, clipboard, drag and drop, remote
  desktop, screen reading, application launch, or shell integration;
- shared pixel buffers, zero-copy presentation, GPU command submission, DMA,
  acceleration, page flipping, vsync, damage tracking, fences, multi-monitor,
  scaling, rotation, HDR, profiles, alpha composition, or color management;
- screenshot/capture authority, telemetry, history, recording, Oracle output,
  or ambient Eidolon tracing;
- supervision, restart, multiple display clients, concurrency, SMP ordering,
  production scheduling, crash recovery, or secure erasure; or
- a complete accessible-neumorphic component system or any WCAG/platform
  accessibility certification.

## Acceptance requirements

Host tests must cover:

- exact size, offset, little-endian decode, format-tag, flag, reserved, and
  supported-body rules for the version-1 header;
- zero and maximum valid dimensions, multiplication/addition overflow,
  inconsistent row/pixel/payload lengths, payload-cap edges, and every
  rectangle-boundary edge;
- synthetic RGB/BGR channel masks, pitch padding, clipped-canary protection,
  exact accepted-region mutation, and complete rejection atomicity;
- Rite right subsets, stale task incarnations, error precedence, exact payload
  limits, sequence behavior, and scratch clearing; and
- ordinary-image exclusion of every feature-only symbol, string, fixture, and
  payload marker.

The QEMU proof must show one RX code page, one RW+NX mailbox, one RW+NX stack,
no user-accessible framebuffer mapping, the exact narrow grant, ordered
rejection and commit counts, terminal exit without returning to the exited
task, complete task/channel/address-space teardown, restored allocator/object
accounting, and no error, fatal, or private-content record. Its captured strict
P6 frame must have the expected 1024x768 RGB8 geometry and match the complete
independent public-fixture oracle.

Acceptance also requires formatting, warning-denied host and x86-64 Clippy,
the established host and QEMU regression matrix, an explicit all-feature link,
and `normal-exclusion`. VirtualBox is not required for this removable QEMU
mechanism proof.

## Acceptance evidence

At implementation checkpoint `b6546cc`, the accepted proof completed all of
the required checks:

- `make check` passed the complete configured formatter, host-test,
  warning-denied host and x86-64 Clippy, freestanding-build, and script-syntax
  matrix. Its directly relevant suites included 22 `anomalous-abi` unit tests
  plus six compile-fail documentation tests, 51 `anomalous-session` unit tests
  plus 56 compile-fail documentation tests, 24 `anomalous-session-router` unit
  tests plus 46 compile-fail documentation tests, 307 focused
  `display_vessel_test` kernel tests, and 46 `xtask` tests.
- `make display-vessel-smoke` accepted exactly 16 structured records: seven
  boot records and nine display-proof records. The ring-3 vessel made 772
  calls: three ordered rejection probes, 768 full-width row commits, and one
  terminal task exit.
- Each denied, malformed, or out-of-bounds probe left every byte of the
  canary-backed framebuffer aperture unchanged. The accepted task had exactly
  one RX code page, one RW+NX mailbox page, one RW+NX stack page, and the
  `display_update | task_exit` grant. An explicit walk of its page root proved
  the complete framebuffer aperture remained supervisor-only.
- The terminal exit did not return to the retired task. The proof withdrew the
  task, channel, three user mappings, private root, copied-pixel scratch, user
  storage, page tables, and transition state, then restored allocator and
  object accounting exactly.
- The host captured the retained 1024x768 strict-P6 frame and an independent
  fixture oracle matched all 786,432 RGB pixels. The interactive `make
  display-demo` command boots that same inspected feature image for human
  review.
- The complete non-VirtualBox QEMU regression matrix passed: `smoke`,
  `fault-smoke`, `circle-smoke`, `control-smoke`, `rite-smoke`,
  `scheduler-smoke`, `supervision-smoke`, `shell-vessel-smoke`,
  `memory-smoke`, `interrupt-smoke`, `input-wait-smoke`, `shell-smoke`, and
  `display-vessel-smoke`, followed by `normal-exclusion`.
- An explicit `--all-features` x86-64 freestanding build and a strict final ELF
  link with fatal warnings and orphan-section rejection passed. Normal-image
  inspection excluded the display gate, fixture, marker, helper symbols, and
  test-only payload.

Independent security, ABI/presenter, and verifier reviews found no remaining
P0 or P1 issue. Their actionable findings were closed before this checkpoint:
full-aperture rejection evidence, exact rectangle-edge and error-precedence
coverage, bounded verifier reads, verify-before-print privacy handling,
workspace-wide proof serialization, a dedicated feature sentinel, and precise
supervisor-only aperture language are all present in the accepted tree.

VirtualBox was not run or required. This decision accepts the bounded QEMU
mechanism proof and its human-review command, not a production graphics stack
or a broader speculative-isolation claim.

## Consequences

Anomalous gains its first visible output produced by a fixed ring-3 task with
no user-accessible framebuffer mapping. The screenshot and interactive image
give human collaborators an early review surface, while the narrow raw-pixel
boundary stays independent of the future semantic desktop model.

The proof is intentionally inefficient: full-frame RGB8 rows require many
Rite calls and privileged pixel conversion. That cost is acceptable evidence
for the ownership and validation boundary, but it is not the design for a
production compositor. A later decision must build the bounded semantic scene;
another must compose it with supervised vessels, input/focus policy, and a
more capable display transport before Anomalous can claim a user-space desktop.

The present task root still borrows the kernel's supervisor-only low-memory
branch, so the framebuffer remains translated for CPL0 even though the U/S
path makes it inaccessible to CPL3. This proof does not claim KPTI or a
Meltdown-class speculative isolation boundary. A later address-space decision
must minimize or eliminate inherited kernel mappings before Anomalous makes
that stronger privacy claim.

## Alternatives considered

### Map the framebuffer into the display vessel

Rejected for this slice. It exposes device layout and unrelated screen content
and permits unrestricted writes outside an owned surface.

### Add a kernel drawing-command language

Rejected. It increases privileged parser and renderer complexity and risks
making kernel commands the long-term UI model.

### Define the full semantic desktop before any native presentation

Deferred, not rejected. That model is essential, but this independent lower
boundary first proves that its eventual renderer can present without raw
framebuffer authority. The presentation ABI is explicitly not the semantic
model.

### Extend the accepted Rite proof in place

Rejected. ADR 0019's shell grant and strict transcript are closed evidence.
The display client needs a distinct narrow grant, payload, visual artifact, and
normal-image exclusion boundary.

### Use GRUB, a hosted window, or an existing desktop stack

Rejected as acceptance evidence. A host preview may help design work later,
but it would not prove Threshold's framebuffer ownership or an Anomalous
ring-3-to-kernel presentation path.
