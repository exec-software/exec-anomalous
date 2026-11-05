---
title: "Native Session Input and Exact Wake Log"
date: 2026-09-13
tags: [development, kernel, session, input, rites, scheduler, privacy, x86-64]
status: accepted
domain: operating-system
---

# Native Session Input and Exact Wake Log

## Starting point

ADR 0021 accepts a host-only private semantic-input and lost-wake-safe session
model. The kernel still has only a physical `input_read` right with a zero-byte
request and payload-free `would_block` result. Rite admission cannot survive a
context switch, the x86-64 transport cannot copy a reply body, and scheduler
wake is coupled to terminal supervision transitions.

The next proof must close those exact mechanism gaps without turning the raw
keyboard queue into a shell input source or pretending that a fixed fixture
authenticates people, machines, devices, accessibility services, paste, or
remote clients.

## Accepted implementation

**Accepted and implemented:** ADR 0022 introduces a distinct
`session_input_read` right and operation. Physical `input_read` remains
separate. The request and delivery are
fixed, versioned bodies with an exact 256-byte semantic text capacity and a
fixed-size outer reply that does not reveal the exact text length.

**Implemented:** one feature-only kernel endpoint binds exact endpoint, session,
consumer, route, task/circle, wait, readiness, and delivery generations. It has
four private FIFO slots, one consumer, and one pending wait. Its deterministic
publisher is a kernel-authored proof fixture, not a deployed router or authority
service.

**Implemented:** an admitted blocking read becomes an owned pending-call record
before the scheduler selects another task. The scheduler gains one exact
`blocked -> suspended` FIFO-tail wake while the publisher remains running;
event authority stays in the endpoint rather than the scheduler.

**Implemented:** publication queues the complete action and enqueues the exact
blocked consumer without copying or consuming it. Only when a prepared
scheduler transition selects that consumer does the kernel acknowledge the
exact wait, invalidate any old marker, copy the complete body, consume and
clear the source, and publish the reply marker last. Route closure before that
selection clears typeahead and converts the same pending wait to payload-free
`interrupted`.

## Revision and atomicity contract

Publication advances the endpoint revision once. When a wait is live, it must
leave four future revisions available. Wait installation advances once;
immediate delivery advances once. A selected input-ready acknowledgement and
source consumption advance twice as one native atomic transition, matching the
ADR 0021 host model's separate acknowledgement and receive transitions without
publishing an intermediate native state. First route closure and terminal
acknowledgement each advance once; repeated closure is idempotent.

The host/native conformance proof establishes a starting revision of
`u64::MAX - 6` as the final complete wait/publication/selected-delivery
frontier. Starting at `u64::MAX - 5` makes that later live-wait publication
reject without mutating either model.

## Security and privacy boundaries

- No scan code, key transition, controller byte, device identity, or physical
  provenance exists on the semantic-session boundary.
- Human and machine fixtures use the same mechanism but retain distinct private
  origin labels. Origin is attribution, never authentication or authority.
- Identities are non-secret freshness values and confer no rights. Request and
  reply content remains private even though its bytes also confer no authority.
- Serial evidence is content-free and omits action/origin tags, exact lengths,
  queue depth, identities, traces, pointers, addresses, markers, and timing.
- Explicit clearing is a logical lifetime property, not secure erasure.
- Focus loss is an externally supplied test reason. This slice neither detects
  focus nor decides policy.

## Accepted fixed proof

1. Queue one fixed human-attributed semantic action and prove immediate native
   delivery and copy-before-consume to the ring-3 consumer.
2. Let the consumer wait on an empty endpoint, settle its synchronous rite into
   owned pending state, block it, and dispatch a peer.
3. Publish one equivalent machine-attributed action from the kernel-authored
   fixture, mark the wait input-ready, and append only the exact consumer to the
   runnable tail while the peer remains current; do not copy or consume yet.
4. Select the consumer with its reserved dispatch sequence, acknowledge the
   exact wait at that boundary, copy before consuming, publish the reply last,
   and validate distinct private attribution plus equal semantic shape.
5. Block once more, close the exact route as an externally decided focus loss,
   clear typeahead, wake once, then terminalize the peer through one expected
   contained `#UD` and validate a payload-free interruption in the consumer.
6. Restore kernel CR3 and TSS state and clear runtime publications before the
   nonlocal original-kernel-stack return. Retire channel and endpoint state and
   clear task storage only after that return, restore accounting, and prove the
   complete feature absent from the ordinary image.

## Acceptance evidence

ADR 0022 was accepted on 2026-09-19 against implementation checkpoint
`fcb3578`.

- `make check` passed with 17 `anomalous-abi` unit tests and 5 compile-fail
  documentation tests; 51 `anomalous-session` unit tests and 56 compile-fail
  documentation tests; kernel host suites with 286 default, 291 circle, 286
  control, 297 rite, 292 scheduler, 292 supervision, 303 shell-vessel, 286
  memory, 286 interrupt, and 303 input-wait tests; and 38 `xtask` tests.
- Formatting, script syntax, warning-denied host and x86-64 Clippy, and an
  explicit all-features bare-metal x86-64 final ELF link passed.
- All 625 four-step host/native differential traces and every action/origin
  boundary agreed. A stale pending Rite preparation rejected a later wait with
  the same session binding but different wait/readiness identity.
- Failure injected at each of the 280 delivery-copy offsets left the reply
  header invalid and exact endpoint/private-queue, full Rite-channel, task,
  scheduler, and retained-selection snapshots unchanged.
- The complete QEMU matrix passed, including `input-wait-smoke`. Its strict
  content-free transcript contained exactly 19 records, 11 from the feature
  lifecycle. Normal-image exclusion also passed.
- Independent correctness/atomicity, scheduler/lifecycle,
  unsafe-boundary/privacy, verifier, precedence, and exclusion reviews were
  clear after the reviewed fixes.

VirtualBox was not run or required. This removable proof does not change
Threshold or the ordinary-image boot path, and its native lifecycle is covered
by QEMU.

## Claim boundary

This is not a secure shell, authenticated input router, physical-input familiar,
focus manager, secure-attention path, Canticle implementation, production
session host, general scheduler, stable ABI, or secure-erasure proof. The native
boundary remains x86-64; IA-32 and 32-bit Win32 translation remain isolated
user-space compatibility-veil work.
