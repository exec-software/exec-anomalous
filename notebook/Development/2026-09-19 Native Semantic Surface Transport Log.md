---
title: "Native Semantic Surface Transport Log"
date: 2026-09-19
tags: [development, desktop, surface, transport, rite, privacy, security, ring3]
status: accepted
domain: semantic-control
---

# Native Semantic Surface Transport Log

## Outcome

ADR 0028 is accepted at implementation checkpoint `228a168`. Anomalous now has
one removable x86-64 proof that copies a fixed, explicitly versioned semantic-
surface body from one ring-3 source into a kernel-owned one-slot endpoint and
then into one separate ring-3 consumer. The endpoint owns an independent copy,
so the source can be retired completely before consumer delivery.

This slice ends at one-time copied delivery. It does not render the body,
complete ADR 0027's render lease, stage or present pixels, prove scanout,
acknowledge the host transaction, or publish semantic revision N+1.

## Wire and authority boundary

The accepted ABI uses two new, non-overlapping Rite rights and operations:

- `semantic_surface_submit = 1 << 6`, operation 7;
- `semantic_surface_read = 1 << 7`, operation 8;
- an exact 16-byte little-endian read request; and
- an exact 528-byte little-endian version-1 snapshot.

The source grant is `semantic_surface_submit | task_exit` (`0x44`), while the
consumer grant is `semantic_surface_read | task_exit` (`0x84`). They run in
separate address spaces with separate RX images, RW+NX mailboxes, RW+NX stacks,
and guard pages. Neither task receives the other transport right, framebuffer
access, console access, physical-input access, or semantic-session input.

Surface, scene, revision, renderer, attempt, task, endpoint, trace, sequence,
and address values remain correlation data. Authority comes from the trusted
live task incarnation, its kernel grant, the private endpoint generation, and
the exact endpoint state.

## Codec and privacy boundary

`anomalous-desktop-transport` is `no_std`, allocation-free, and unsafe-free.
Its core decoder checks the complete structural wire format without importing
scene or service policy. An optional adapter projects ADR 0027's borrowed,
already-redacted view field by field; it does not serialize the private Rust
`render_snapshot`, enum layouts, padding, brands, or move-only authority.

The consumer has no scene, service, adapter, or encoder dependency. Its
checked-in oracle independently verifies the complete fixed profile, including
the binding, state, labels, and all 25 commands.

Two source builds used runtime-derived protected inputs with different values
and lengths. They produced byte-identical 528-byte snapshots and byte-identical
source ELFs. The protected inputs were recreated only in bounded host memory,
zeroed, and scanned across generated Cargo outputs, ELFs, flat images, wrapper
objects, the feature kernel, the ordinary kernel, and the transcript; no
occurrence was found. This proves content and exact-length noninterference for
the fixture. It does not hide activity, timing, ordering, denial of service, or
resource pressure.

Inspected artifacts were:

- source flat image: 2,384 bytes, one page;
- source ELF: 1,107,792 bytes,
  `8efcac631ad19bff57938b5f1aac82d4b3d9df0c2a6009bff05e8f936d6c0046`;
- generated snapshot: 528 bytes,
  `97a7e5a8544f2a8a3584beb6df6d1f6052627184df7469c8674545c7fa41871b`;
- consumer flat image: 5,094 bytes, two pages; and
- consumer ELF: 1,185,928 bytes,
  `79076d34284168f8c7b3452e8d7610f6a53cba44b60fa89dc7d001a9cc654f94`.

## Transaction and fault behavior

Submission copies the admitted source body into kernel scratch before parsing.
All fallible structure, correlation, freshness, counter, reply, and ownership
checks complete before the endpoint commit. The one-slot endpoint then owns the
complete 528-byte value; full backpressure returns the competing owner without
changing the queued body.

Delivery validates the exact request and destination span, clears stale reply
storage, stages the complete body and reply prefix, commits consumption of the
exact endpoint owner, and publishes the reply header last. Empty delivery is
payload-free and leaves the endpoint unchanged.

Host fault models inject failure at every one of the 528 source-copy offsets
and every one of the 528 destination-copy offsets. Each failure preserves the
endpoint, channel, task, binding, counters, mailbox publication state, and
source ownership expected at that precommit boundary. Counter-exhaustion,
malformed, stale, foreign, full, empty, close, replay, and teardown paths are
also failure-atomic. Clearing is logical lifetime evidence, not secure erasure.

## Native lifecycle evidence

The strict QEMU proof executes this order:

1. source read-right denial;
2. malformed submit rejection;
3. exact `wrong_surface` freshness rejection after Rite admission;
4. one valid submit;
5. one full-slot backpressure result;
6. terminal source exit and complete source resource withdrawal;
7. consumer submit-right denial;
8. malformed read rejection;
9. one complete independently validated delivery;
10. one empty read; and
11. terminal consumer exit and exact endpoint/resource teardown.

The accepted transcript reports 11 Rite calls, 9 body copies totaling 2,176
bytes, one committed submission, one delivery, one backpressure result, two
Rite authority denials, two malformed results, two exits, and two restored
kernel continuations. The admitted freshness rejection is proven separately
and is not counted as an authority denial.

The source is retired before delivery while the endpoint retains the copied
body. Delivery consumes that body once, publishes the destination header last,
and clears stale reply bytes. Final evidence restores the exact initial frame,
lease, task, context, scheduler, channel, endpoint, mapping, address-space,
scratch, CR3, TSS stack, and task-switched accounting.

## Acceptance validation

`make check` passed, including formatting, all configured host suites, 359
semantic-feature kernel tests, 59 strict `xtask` tests, the 13-test transport
and adapter suite, warning-denied host and freestanding x86-64 Clippy, isolated
artifact inspection, both feature-precedence links, and normal-image exclusion.

The serial non-VirtualBox QEMU matrix passed:

- `smoke`, `fault-smoke`, `circle-smoke`, `control-smoke`, and `rite-smoke`;
- `scheduler-smoke`, `supervision-smoke`, and `shell-vessel-smoke`;
- both configured `memory-smoke` sizes, `interrupt-smoke`,
  `input-wait-smoke`, and `shell-smoke`;
- `display-vessel-smoke` and `supervised-renderer-smoke`; and
- the dedicated `semantic-surface-transport-smoke`, followed by a fresh
  `normal-exclusion`.

VirtualBox was not run or required. ADR 0028 adds a removable feature proof and
does not change Threshold or the ordinary-image boot path.

## Next boundary

The next logical slice must consume this delivered body through the supervised
renderer, establish actual framebuffer presentation evidence, and return an
exact acknowledgement and presentation result to ADR 0027 before semantic
publication. Window lifecycle, focus, input/action policy, accessibility
service deployment, display-manager policy, capture policy, and general
composition remain separate decisions.
