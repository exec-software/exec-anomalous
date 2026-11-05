---
title: "Transactional Semantic Surface Publication Log"
date: 2026-09-19
tags: [development, desktop, scene, surface, renderer, privacy, security]
status: accepted
domain: semantic-control
---

# Transactional Semantic Surface Publication Log

## Outcome

ADR 0027 is accepted. The host-only `anomalous-desktop-service` proof now keeps
one canonical committed revision public while exactly one successor may be
queued, leased to a renderer, returned ready, or held behind a trusted
presentation receipt. The successor becomes the public human, accessibility,
and machine state only when `desktop_surface::publish` consumes that exact
receipt.

This slice settles local ownership, freshness, strict backpressure, one bounded
renderer replacement, and the semantic publication point. It does not define a
native scene transport, compositor, window protocol, input path, renderer, or
stable ABI.

## Implemented contract

The service is `no_std`, allocation-free, unsafe-free, absent from the kernel
and normal image, and owns one exact ADR 0025 scene plus its private scene
control. It accepts one fixed profile:

- a 1024x768 logical surface;
- eight canonical nodes and two actions;
- seven exact declared-public structural labels;
- `System ready` or one protected source in the status slot, with protected
  source projected as `Protected status`; and
- 25 standard-depth or 21 flat-depth paint commands.

ADR 0027 adds only two narrow facilities to the ADR 0025 scene crate:
`prepared_revision::preview()` provides a borrowed, authority-free, already
redacted render preview, and `prepared_revision::into_candidate()` recovers a
candidate from a preparation still owned by its caller. Neither operation
cancels an installed service revision or changes committed scene semantics.

`desktop_surface::new_presented` treats revision one and publication sequence
one as already presented by a trusted host. It returns distinct move-only
surface and presentation controls plus one move-only renderer endpoint. Public
identities, revisions, sequences, attempts, selectors, geometry, and receipt
fields are correlation data rather than authority.

## Lifecycle

The declared success path is:

`steady -> queued -> leased -> ready -> presented -> steady`.

| Phase | Held private progression | Allowed forward operation |
| --- | --- | --- |
| `steady` | No successor | `submit_revision` installs one complete successor |
| `queued` | Prepared scene plus immutable snapshot | `acquire_render` mints one exact lease |
| `leased` | First or replacement renderer owns progress | `complete_render`, or the one authorized restart |
| `ready` | Move-only `prepared_presentation` exists | `attest_presented`, or the one authorized restart |
| `presented` | Move-only `presentation_receipt` exists | `publish`; restart is forbidden |

Successful publication commits the prepared scene, exposes the new accessible
render plan, accessibility tree, and machine projection together, advances the
publication sequence, consumes the receipt, and releases the pending slot.
Until then the committed typed observation remains exactly unchanged.

There is no pending-revision cancellation. Whole-surface retirement under the
exact surface control is the only alternative release path. Dropping a lease,
prepared presentation, receipt, endpoint, or control performs no protocol
transition. Losing a presentation receipt strands the service in `presented`
until retirement; the proof cannot withdraw external pixels after the host has
asserted their presentation.

## Backpressure and replacement

`submit_revision` derives the complete paired projections and the renderer
snapshot before changing service state. Every occupied phase returns
`pending_full` and the complete competing candidate. Malformed, invalid,
foreign, stale, wrong-control, and exhausted submissions likewise return the
caller's owner and leave the current service unchanged.

One renderer replacement is allowed for the entire surface lifetime, and only
from `leased` or `ready`. It preserves the exact pending candidate and typed
snapshot fields, advances to a fresh renderer generation, returns a fresh
endpoint, and uses delivery attempt two. The old endpoint, lease, or prepared
presentation remains stale. A second replacement returns
`restart_limit_reached`. Queued and presented work cannot restart.

## Snapshot and privacy boundary

The private `render_snapshot` has fixed-size storage and a compile-time size
assertion of at most 1,024 bytes. It carries fixed typed identities, state,
tokens, rectangles, slots, active state, and nine fixed label selectors. The
accepted profile makes a runtime oversized snapshot unrepresentable. Tests
therefore cover the static cap, both command counts, and out-of-range access;
they do not claim a serialized byte layout.

Protected source bytes and exact lengths never enter the snapshot, scalar error
codes, delivery owners, or receipts. Tests compare the complete defined typed
field view, not raw Rust object bytes or padding. Candidate-returning rejection
envelopes deliberately contain the caller's original private owner so it can be
retried or cleared; this is distinct from the static content-free
`surface_error`.

The privacy claim is content and exact-length non-disclosure, not activity
noninterference. Direct calls and correlation fields expose phase, revision,
publication sequence, renderer, attempt, resource pressure, geometry, and
host-added timing or cadence. The crate creates no ambient observation type,
log, telemetry sink, screenshot, crash artifact, or history.

## Limited action and focus facade

The service keeps ADR 0025 projection owners private but can mint service-
branded action and focus handles from the currently committed accessible or
machine projection. `resolve_action` and `traverse_focus` validate these handles
without performing an action effect, mutating scene focus, or routing input.

Revision-N handles remain valid while N+1 is queued, leased, ready, or
presented. They become stale only after publication commits N+1. Equal public
references from another private service brand confer no authority.

## Evidence

Focused host evidence passed:

- 19 scene unit tests, 136 scene documentation/compile-fail tests, and
  warning-denied scene Clippy;
- 14 service unit tests, 74 service documentation/compile-fail tests, and
  warning-denied service Clippy;
- all 64 valid and 32 invalid constructors;
- all 32 invalid successor states with exact candidate return and unchanged
  committed observation;
- all `64 * 64 = 4,096` valid state transitions through queued, leased, ready,
  presented, and publication;
- strict backpressure in every occupied phase, repeated immutable reads,
  restart typed-field equality, stale and foreign owner rejection, freshness
  exhaustion paths, protected-source equality, and action/focus staleness; and
- directed coverage of every declared lifecycle phase and retirement path.

The lifecycle evidence is directed and bounded, not exhaustive model checking
or a counted enumeration of every possible call sequence. Receipt uniqueness is
enforced by move-only safe-Rust construction and consumption; runtime tests use
foreign and stale owners rather than fabricating an impossible duplicate.
Likewise, the proof does not claim safe-API exhaustion of the private process-
wide service brand or construction of an impossible third delivery attempt.

Complete acceptance evidence also passed:

- `make check`, including formatting, the configured host suite, 329 all-
  feature kernel tests, 56 xtask tests, renderer base and scene-conformance
  tests, artifact inspection, all-feature freestanding linking,
  `normal-exclusion`, script syntax, and warning-denied host and x86-64 Clippy;
- the established QEMU matrix, serially: `smoke`, `fault-smoke`,
  `circle-smoke`, `control-smoke`, `rite-smoke`, `scheduler-smoke`,
  `supervision-smoke`, `shell-vessel-smoke`, both configured `memory-smoke`
  sizes, `interrupt-smoke`, `input-wait-smoke`, `shell-smoke`,
  `display-vessel-smoke`, and `supervised-renderer-smoke`;
- a final `normal-exclusion`; and
- `cargo tree -p kernel`, which contains no `anomalous-desktop-service`.

Implementation checkpoint: `2076eba`.

VirtualBox was not run. The QEMU results are unchanged native regressions; they
do not claim that this host-only service executed in an emulator.

## Next boundary

The next display decision may define a copied, versioned native scene/surface
transport and bind real task, surface, supervision, and presentation authority.
It must not serialize this private Rust snapshot by convention or reinterpret
the trusted host receipt as proof of pixels, scanout, renderer integrity, or
secure attention.
