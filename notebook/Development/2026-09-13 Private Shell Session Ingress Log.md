---
title: "Private Shell-Session Ingress and Wait Log"
date: 2026-09-13
tags: [development, shell, session, input, privacy, security, waiting]
status: accepted
domain: semantic-control
---

# Private Shell-Session Ingress and Wait Log

## Starting point

ADR 0020 proves that a fixed supervised ring-3 shell-shaped vessel can make
native rites across preemption and destructive restart. Both input calls see an
empty source and return `would_block`. The proof deliberately supplies no
nonempty input, session host, general wait, machine command encoding, or
Canticle evaluator.

The next risk is not syntax. It is defining what private input belongs to, how
it reaches one session, and how a consumer sleeps without losing a delivery or
accepting a stale wake.

## Accepted experiment

**Accepted and implemented:** ADR 0021 adds a host-only
`anomalous-session` crate. It models one bounded session, one exact consumer,
one route epoch, a four-entry semantic action queue, and one lost-wake-safe
wait. It is not linked into the kernel and does not alter `anomalous-abi`.

The proof input is semantic editing data:

- bounded private UTF-8 insertion;
- explicit erase, submit, cancellation-of-submission, and discontinuity
  actions; and
- distinct attribution for human text, machine text, paste, accessibility, and
  remote text.

There is no raw key or physical-input variant. Text insertion never submits,
even when a pasted fragment contains a line feed. Origin remains attribution,
not authentication or authority. The origin-stamping entry point belongs only
to a trusted router; the enum itself authenticates nothing.

## Privacy contract

The state machine makes the bounded privacy path directly testable:

- content is private and ephemeral by default;
- no history or persistence interface exists;
- private owner types and local receipts cannot be cloned, formatted, or hashed
  implicitly, while named content, length, and count access is an explicit
  local disclosure;
- errors carry static codes rather than excerpts;
- privacy-limited observations contain no content, identifier, exact length,
  origin, timing, or content-derived digest and authorize no telemetry sink;
- queue pressure returns the private value rather than silently dropping it;
- explicit close clears queued content and reports the discarded-entry count;
  and
- explicit clear, consumption, close, and `Drop` share one logical-clear path.

Logical clearing is not a claim that external copies, compiler temporaries,
caches, swap, or hardware state were securely erased. `Drop` is only a privacy
fail-safe because it cannot return a close receipt or terminal wake. The trusted
host must explicitly close, deliver, and acknowledge before owner destruction;
abrupt host loss remains outside this proof.

## Wait contract

A receive preparation captures the exact session, consumer, route, next wait
sequence, and revision. Commit blocks only if the same session remains open and
empty at the same revision. Publication between those operations makes commit
return ready. Publication after blocking enqueues first and then returns the
exact wake identity.

A copied wake identity confers no authority: one exact equal value may
complete the current wait and all remaining copies then become stale. Wait
cancellation applies only before a wake becomes pending. Close must preserve the
exact blocked identity as one terminal tombstone, clear and count queued
typeahead, and carry neither into a checked same-lineage replacement.

Each successful publish, receive, wait install, acknowledgement, cancellation,
and first close advances one checked revision. Ordinary mutations reserve two
values for close plus possible terminal acknowledgement so exhaustion cannot
strand an accepted queue or waiter. Replacement requires a closed,
fully-acknowledged owner and advances session, consumer, and route freshness.
The session slot and its complete generation namespace must be reserved as one
exclusive lineage by the trusted host because this crate has no global registry.

## Kernel boundary deliberately deferred

The current kernel `input_read` request has no reply capacity and can only
finish `would_block`. Rite admission is a synchronous guard, the x86-64 transport
has no accepted reply-body transaction, and scheduler wake is coupled to fixed
supervision transitions. This experiment must not disguise those gaps.

A later kernel ADR will need a fixed nonempty reply schema, copy-before-consume,
a readiness generation, an owned wait record, exact wake/revocation behavior,
and complete teardown. Raw physical transitions stay private to the input
mechanism; a future input familiar routes authorized semantic actions.

## Acceptance evidence

ADR 0021 was accepted at implementation checkpoint `a2c72b2`. The observed
acceptance matrix was:

- 8 `anomalous-abi` tests;
- 51 `anomalous-session` unit tests plus 56 independent compile-fail
  documentation tests;
- the `anomalous-session` `no_std` library check and warning-denied rustdoc;
- kernel host suites with 235 default, 240 circle, 235 control, 240 rite,
  241 scheduler, 241 supervision, 246 shell-vessel, 235 memory, and 235
  interrupt tests;
- 35 `xtask` tests;
- formatting, script-syntax checks, and warning-denied Clippy for both host and
  x86-64 target configurations;
- every established QEMU target: `smoke`, `fault-smoke`, `circle-smoke`,
  `control-smoke`, `rite-smoke`, `scheduler-smoke`, `supervision-smoke`,
  `shell-vessel-smoke`, `memory-smoke` at both 96 MiB and 320 MiB,
  `interrupt-smoke`, and `shell-smoke`; and
- `normal-exclusion`.

The final independent privacy/security and correctness audits were clear.
VirtualBox was not run and was not required because this slice changed no boot
or kernel code.

The accepted evidence remains deliberately narrow. It establishes the bounded
host state machine, not secure erasure, authenticated input or authority,
nonempty kernel input delivery, a kernel wait interface, Canticle, a secure
shell, or production confidentiality.

## Claim boundary

This is not a secure shell, parser, Canticle, machine AST, authenticated input,
focus manager, history service, kernel wait API, or stable ABI. It is a bounded
host proof intended to make the later privileged boundary smaller and
falsifiable.
