---
title: "Authenticated-Source Session Routing Log"
date: 2026-09-19
tags: [development, session, routing, authentication, focus, privacy, security]
status: accepted
domain: semantic-control
---

# Authenticated-Source Session Routing Log

## Starting point

ADR 0021 accepts one private host session with bounded semantic actions,
move-only storage, exact waiting, and orderly close/replacement. Its origin
stamping is intentionally a trusted-router operation rather than
authentication.

ADR 0022 accepts the fixed native mechanism needed to return one nonempty
semantic action and wake one exact blocked consumer. Its publisher remains a
kernel fixture, and its version-1 body carries action and origin but no
principal or source identity.

The remaining host-side question is narrower than a production authenticator:
can a bounded router consume an already-authenticated host assertion, prevent
the source from choosing its origin or focus, and prevent a prepared private
action from crossing revocation or human focus transfer?

## Proposed experiment

ADR 0023 initially proposed a separate `no_std`, allocation-free, unsafe-free
`anomalous-session-router` crate. The router would own exactly two
`shell_session` routes, A and B, plus two fixed source slots. It would expose no
inner-session escape path.

Construction would return one move-only router control. Only that owner could:

- admit an externally authenticated principal/source assertion;
- bind one human source to the current human focus and one machine source
  explicitly to route B;
- authorize the one A-to-B human focus transfer; and
- revoke or retire the route.

Successful admission would create a move-only source authority bound to the
exact router, principal, source generation, origin, and next request sequence.
Principal, source, and request numbers would remain non-secret freshness and
attribution data rather than bearer authority.

The human source would begin on A and follow the exact focus transfer to B. The
machine source would remain explicitly bound to B and would never acquire or
change authority because human focus moved. Once a target route is resolved,
both sources would use the same private preparation, publication,
backpressure, wait, and receive machinery with distinct attribution.

This vocabulary is deliberately conventional. The proof is not a familiar
because it mediates no device protocol, not a vessel because it has no service
lifecycle, and not a sigil or rite because it defines no platform authority or
wire format.

## Proposed publication transaction

Publication would have two explicit phases:

1. Preparation revalidates the source and routing mode; resolves human focus or
   exact machine route B; captures the source's current exact-next nonwrapping
   request sequence; stamps the fixed admitted origin; and returns a move-only
   routed preparation without consuming the sequence.
2. Publication revalidates the complete principal/source, applicable focus,
   session, consumer, route, and request correlation before entering the
   accepted ADR 0021 queue transition.

Revocation between those phases must reject without changing either private
session. The A-to-B focus transfer must make a prepared human-A action stale,
while a machine preparation for unchanged B remains valid. Queue pressure and
other session rejection must return the complete private owner with the same
retryable sequence. Queue pressure may retry the same preparation; stale human
focus requires a fresh preparation against B but returns the action and leaves
its source sequence unconsumed. Only successful session publication advances
that per-source cursor. It does not replace the session submission sequence or
grant authority.

## Proposed retirement rule

The fixed focus transition is
`focused(A) -> focused(B)`. The move-only focus authority would preflight live,
open, empty B and the next focus generation, then close A, clear and count its
queued typeahead, and publish the new B authority. If A has a waiter, the close
receipt preserves its exact terminal tombstone for acknowledgement before A is
replaced or destroyed.

Route B remains live and unchanged. A machine-B preparation therefore remains
valid and retryable throughout the transfer. That distinction is the core
security claim: human typeahead and preparation cannot cross focus, while
machine authority cannot be acquired or revoked merely by moving human focus.

The focus decision itself remains external input to the trusted router control.
The proposed proof does not observe desktop focus, arbitrate policy, or provide
a secure-attention path.

## Privacy boundary

Principal/source attribution would remain local and move-only. Routed
preparations and publication receipts may expose that attribution through named
local disclosure methods, but the underlying session delivery and ADR 0022's
version-1 native body remain unchanged and continue to carry only action and
origin.

Errors and observations must omit content, action and origin tags, exact
lengths and counts, principal/source/session/route/request identities, timing,
excerpts, and digests. Logical clearing remains a lifetime property, not a
secure-erasure claim or permission to emit telemetry.

## Intended evidence

The host suite should cover:

- both fixed source slots and both exact session routes;
- wrong, stale, revoked, exhausted, replaced, and cross-router authority;
- equal human/machine routing with distinct attribution;
- a source's inability to choose origin or focus;
- the exact A-to-B transfer on empty, queued, prepared, and blocked A;
- stale human-A preparation alongside still-valid machine-B preparation;
- exact A terminal acknowledgement before A replacement and close-before-focus
  publication;
- nonwrapping request correlation and failure-atomic exhaustion;
- bounded action traces through admission, selection, preparation,
  publication, revocation, retirement, acknowledgement, receive, and
  replacement; and
- compile-fail ownership/privacy tests for every move-only controller,
  authority, preparation, rejection, delivery, and router owner.

If implemented, the crate should enter `make check` with package tests and
warning-denied Clippy. The complete existing QEMU matrix and normal-image
exclusion remain regression evidence, but this host-only proposal needs no new
QEMU feature or VirtualBox run.

## Implementation checkpoint

The host-only `anomalous-session-router` crate now implements the bounded proof
without entering the kernel, ABI, vessel graph, or normal image. It owns A and
B directly, requires live human admission before creating fixed A focus,
permits only the one A-to-B transfer, and binds machine admission to fixed B
without a route selector. Controller revocation selects a fixed source slot by
kind and does not depend on retaining or recovering the source token.

Security review changed four parts of the first implementation before its
checkpoint:

- every router now receives a private, nonwrapping process-local instance
  brand, so a second router with the same public identity cannot mint authority
  for the first;
- construction rejects A/B aliases from the same session-slot or consumer-value
  lineage, while the trusted host remains responsible for reserving those
  complete lineages across separate live routers;
- machine routing is fixed to B and machine-principal reuse requires B
  close-and-replace, eliminating route hopping as a reuse bypass; and
- once human focus reaches shared B, focus revocation returns
  `shared_route_active` until control explicitly revokes the machine binding.

The focused evidence currently passes 24 router unit tests, 46 independent
compile-fail ownership/privacy cases, and warning-denied all-target Clippy. The
unit suite includes every semantic action through both source classes, exact
wait/wake parity, equal backpressure retry, stale and cross-router owners,
controller revocation, request/session/revision exhaustion, failure-atomic
transfer close rejection, shared-B revocation ordering, terminal-wake
acknowledgement before replacement, and static formatting of every router
error.

The exhaustive subset is deliberately narrower than the directed lifecycle
suite: all `7^4 = 2,401` four-step post-admission traces over immediate human
prepare/publish, immediate machine prepare/publish, one transfer attempt, A
receive, B receive, machine revocation, and eligible B
retire/replace/re-admit. Admission, held preparations, human/focus revocation,
wait lifecycle, and A replacement are covered by directed tests rather than
claimed as part of that exhaustive alphabet.

## Acceptance evidence

At implementation checkpoint `83caf3f`, the final acceptance pass completed:

- `make check` passed the complete host, feature-kernel, warning-denied Clippy,
  x86-64 freestanding, formatter, and script-syntax matrix;
- `make smoke fault-smoke circle-smoke control-smoke rite-smoke
  scheduler-smoke supervision-smoke shell-vessel-smoke memory-smoke
  interrupt-smoke input-wait-smoke shell-smoke normal-exclusion` passed;
- the router suite passed 24 unit tests, including all 2,401 declared bounded
  traces, plus 46 compile-fail ownership/privacy cases; and
- the normal image excluded every feature-test probe.

No router-specific QEMU case was added and VirtualBox was not run because the
accepted crate changes no boot, kernel, ABI, vessel, or image path. The existing
emulator matrix is regression evidence only. ADR 0023 is now Accepted.

## Claim boundary

**Accepted bounded host proof:** the crate and local authorities exist, but they
do not authenticate a password, passkey, biometric, person, machine, model,
device, accessibility service, paste source, or remote peer. They do not
provide a physical-input familiar, focus policy, secure attention, native
principal transport, Canticle, a production session host, or a secure shell.
