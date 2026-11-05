---
title: "Native Semantic Presentation Composition Log"
date: 2026-09-20
tags: [development, desktop, surface, renderer, presentation, supervision, privacy, security, ring3]
status: accepted
domain: semantic-control
---

# Native Semantic Presentation Composition Log

## Outcome

ADR 0029 is accepted from implementation checkpoint `2f453e7`, with its final
acceptance-evidence hardening at `e605f54`. Anomalous now has one removable
x86-64 proof that keeps an actual ADR 0027 successor private, transports its
exact ordinal-61 ADR 0028 body through native copied ownership, renders it in an
isolated ring-3 renderer, stages and verifies a complete frame, and only then
consumes the retained private controls to publish coherent human,
accessibility, and machine semantics at revision/sequence 2/2.

This is a one-successful-attempt conformance bridge. It is not a deployed
desktop service, general compositor, window server, cross-process presentation
protocol, native retry path, or stable graphics ABI.

## Composed boundary

The accepted run composes these previously separate boundaries without moving
scene or publication policy into ring 0:

1. the host coordinator retains one live ADR 0027 transaction at presented
   revision and publication sequence 1/1;
2. its exact successor is encoded as one 528-byte ordinal-61 ADR 0028 body;
3. a narrowly granted ring-3 source submits that body into the kernel-owned
   endpoint and is completely retired;
4. a distinct ring-3 renderer with only
   `semantic_surface_read | display_update | task_exit` reads the copied body,
   validates the complete profile, and renders it directly from typed transport
   state;
5. the kernel accepts exactly 768 ascending full-width rows into one zeroed,
   supervisor-only 576-frame stage;
6. terminal successful completion is correlated with the exact delivery,
   renderer incarnation, Rite channel, supervisor child, and completed stage;
7. fallible native state is withdrawn before one complete framebuffer commit,
   full readback, stage clearing, lease release, and exact accounting
   restoration;
8. an independent host oracle checks all 786,432 RGB pixels; and
9. only the resulting private verifier token permits the retained ADR 0027
   owners to complete, attest, and publish exactly once at 2/2.

Public IDs, hashes, serial records, screenshots, and matching scalar fields are
evidence and correlation only. None reconstructs or mints the privately branded
ADR 0027 authority retained by the host coordinator.

## Native lifecycle evidence

The dedicated successful QEMU run emitted exactly seven feature records:
`ready`, `source_released`, `render_acknowledged`, `prepared`, `presented`,
`released`, and `passed`.

Its final counters were:

- 777 Rite calls;
- 775 body copies totaling 2,392,160 bytes;
- 768 ordered row commits;
- one submission and one delivery;
- one backpressure result;
- two authority denials, one malformed result, and one freshness denial;
- two task exits and one successful-exit observation; and
- one complete framebuffer commit.

The kernel compared every visible framebuffer channel byte with the completed
stage and preserved the declared baseline padding. It then cleared and released
the 576-frame stage and restored all 65,095 initial free frames, with zero
owned data frames, table frames, or active leases. The independent host oracle
subsequently matched all 786,432 pixels before semantic publication.

The retained strict-P6 capture was 2,359,312 bytes with SHA-256
`4ada99e89a6beb3966a093b493b03e49c81b32de64c609ae457c00a56b76302c`.
The closed-world serial log was 2,572 bytes with SHA-256
`a19dcc7b1e5f43e99a07f70f863c9afa852f659393a43de5028dd52c6b5f0e6c`.

## Inspected artifacts

The retained ordinal-61 snapshot was 528 bytes with SHA-256
`831773c709ce0a651a7e3b4d4e9549465f7864f284356d074d7bb2d30041cb54`.
The separately inspected native artifacts were:

- source ELF: 1,107,792 bytes,
  `a2bcd57a22801fe821ddb089d0ecfefa2efd137facf28756878672aa1ef0c9a9`;
- source flat image: 2,384 bytes in one page,
  `8bab87d92fa74a43c47321c13743bb371c176169b83cf32bcb9f089a2723f645`;
- source wrapper: 3,848 bytes,
  `3ac25fc3177f11254ab037584a135be57a139fa6734842042eb6d35a09063bad`;
- renderer ELF: 1,239,056 bytes,
  `fcc9decbaddb8122063ac88a52c7dfd1da26b27b287fd4b926b1d4b491bb33ac`;
- renderer flat image: 10,352 bytes in three pages,
  `666c638bd52d76b4883ae0b4bb2a4892fb02fcba0e572440db49ab950e4edc74`;
  and
- renderer wrapper: 11,744 bytes,
  `74693a532c34d19141e6ebf0d0d240b04d98a5c5117495504ae534f6e25a5573`.

The source and renderer remained separately linked fixed-address images. The
renderer had no framebuffer, stage, scene, focus, action, presentation,
publication, capture, allocation, console, physical-input, or semantic-input
authority. The framebuffer and private stage remained supervisor-only.

## Acceptance validation

Acceptance verification covered formatting, warning-denied host and x86-64
Clippy, target dependency and ELF inspection, wrapper and linked-payload
inspection, transcript and private-verifier mutation tests, all-feature
precedence and strict final linking, the complete configured QEMU regression
matrix, the dedicated display-vessel, supervised-renderer,
semantic-surface-transport, and semantic-presentation smoke targets, and final
normal-image exclusion.

VirtualBox was not run or required. This removable proof does not change
Threshold or the ordinary-image boot path.

## Honest limits

The native QEMU evidence exercises the successful renderer path only. Early
exit, partial-stage completion, stale or foreign identity, wrong outcome, and
wrong supervision phase are rejected by structural completion predicates and
architecture-neutral component tests; there is no separate negative x86-64
early-exit QEMU run.

The initial framebuffer is a declared complete-aperture mutation baseline. It
was proven unchanged until the final copy, but it is not a claimed or tested
visual rendering of ADR 0027 revision 1. The final evidence establishes
framebuffer contents under the bounded software model, not hardware scanout,
page-flip completion, vsync, tear freedom, a GPU/DMA fence, or power-loss
atomicity.

The framebuffer copy and host publication remain separate operations. A host
failure between them can leave revision-2 pixels visible while semantic readers
still observe revision 1. The proof neither closes that crash gap nor provides
native renderer retry. It also does not deploy a general desktop, native scene
service, accessibility service, window manager, compositor, display manager,
or stable ABI.

## Next boundary

The next logical slice is a deployed native semantic/window/compositor service
and a cross-process presentation protocol that carries acknowledgement without
turning public bytes into authority and that explicitly addresses the
pixels-versus-semantics crash gap. Focus, input, action, and window lifecycle
should follow as separate policy and authority work.
