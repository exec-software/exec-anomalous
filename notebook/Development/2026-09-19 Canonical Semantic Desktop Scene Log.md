---
title: "Canonical Semantic Desktop Scene Log"
date: 2026-09-19
tags: [development, desktop, scene, accessibility, ai, privacy, security]
status: accepted
domain: semantic-control
---

# Canonical Semantic Desktop Scene Log

## Starting point

ADR 0024 accepts a lower presentation mechanism: one fixed ring-3 task can
submit validated RGB8 patches without mapping the framebuffer. Its public
desktop-shaped image is exact visual evidence, but its pixels intentionally
carry no focus, action, accessibility, identity, consent, or machine-control
authority.

The next question is whether Anomalous can represent a small desktop state once
and derive every client-facing view from the same committed revision. The proof
must make divergence observable before introducing windows, input routing, or
action execution.

## Accepted experiment

ADR 0025 accepts a dependency-free, host-only
`anomalous-desktop-scene` crate. It is `no_std`, allocation-free, and
unsafe-free, and remains absent from the kernel, ABI, native image, and ADR
0024 transport.

The proof uses eight exact semantic slots in canonical preorder within a fixed
1024x768 logical scene: root, masthead, navigation, primary action, secondary
action, content, heading, and status. One complete definition commits
atomically at a nonwrapping revision. A replacement is prepared and fully
projected without changing the live owner, then becomes visible through one
revalidated swap.

Every committed revision borrows as one paired projection:

- a human render plan with semantic paint tokens and a first-class
  accessibility tree; and
- a machine projection with the same nodes, roles, labels or redaction,
  enabled state, focus, and exact revision-bound actions.

No independently authored accessibility sidecar or machine-only action set is
accepted.

## Bounded state matrix

The complete declared UI-state matrix is:

- four primary/secondary enabled combinations;
- focus at none, primary, or secondary;
- declared-public or protected status text;
- standard or flat depth; and
- standard or reduced motion.

That yields 96 candidates. Sixty-four are valid. Thirty-two focus a disabled
control and must reject without advancing the revision or changing either
projection.

Standard depth has an exact 25-command plan. Flat depth removes only four
decorative highlight/shadow commands and retains 21. Both modes retain all
labels, borders, control outlines, state markers, and focus evidence. The
accessibility and machine projections each contain exactly eight nodes; the
machine projection contains exactly two ordered action descriptors.

## Privacy and authority boundary

Declared-public text is a caller assertion and local disclosure boundary, not
proof that data is nonsecret or permission to log it. Protected text remains
owned and move-only; both projections receive one constant redaction token.
Different valid protected byte contents and lengths under otherwise identical
state cannot affect geometry, order, counts, errors, observations, or
focus/action mechanics. Only the nonactionable status slot may be protected in
this first proof.

Public scene, node, action, revision, coordinate, role, and focus values are
selectors or data, never authority. Local resolution requires a separate
move-only handle bound to an unexposed scene instance and current revision. It
returns semantic metadata only and executes nothing.

Scene focus is recorded state and deterministic traversal, not policy. It is a
different type and authority domain from the session router's focus and cannot
select an input route.

## Intended evidence

Acceptance requires the exhaustive 96-state matrix, exact independent
projection oracles, stale/foreign/competing transaction tests, nonwrapping
revision and retirement evidence, protected-label noninterference, deterministic
focus and action parity, fixed palette contrast tests, ownership/privacy
compile-fail cases, warning-denied Clippy, the complete host suite, all existing
non-VirtualBox QEMU regressions, and normal-image exclusion.

## Acceptance status

**Accepted at checkpoint `2857d8c`.** The implementation is a bounded semantic
contract, not a compositor, window server, renderer, input router, action
executor, stable ABI, or normal-image desktop.

## Observed evidence

- All 96 declared construction candidates were enumerated: exactly 64
  committed and 32 focus-disabled candidates rejected. Separate malformed and
  invalid replacement tests proved failure-atomic preparation against an
  existing committed scene.
- Eighteen unit tests and 129 compile-fail documentation tests passed. The
  latter cover every sensitive owner plus raw action and focus references that
  must not substitute for privately branded local handles.
- Independent oracles verified the exact 25/21 paint budgets, eight accessible
  nodes, eight machine nodes, two action descriptors, every valid-state token
  and shape, fixed contrast requirements, and protected-label
  noninterference.
- The full configured host check, warning-denied host and x86-64 freestanding
  Clippy, formatting, script syntax, and diff checks passed.
- Every established QEMU regression from `smoke` through
  `display-vessel-smoke` passed serially, and the unchanged `normal-exclusion`
  regression passed. The kernel dependency tree separately confirms that the
  scene crate is not linked into the ordinary image. VirtualBox was not run or
  required for this host-only boundary.

## Next experiment

Place the accepted scene contract inside one bounded supervised user-space
renderer and feed its semantic paint plan through ADR 0024's presentation
mechanism. Keep window lifecycle, input/focus policy, action invocation,
authentication, fonts, and general layout outside that composition proof.
