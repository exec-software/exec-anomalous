# ADR 0037: Batch ready desktop pointer motion

- Status: Proposed
- Date: 2026-09-26

## Context

The native workspace presents and acknowledges a complete frame before reading
the next input. Faster composition, bounded word transfers, and interactive KVM
reduce its cost, but one full frame per physical motion packet can still build
a queue during sustained input or host load. A pointer must catch up without
losing button transitions, hiding sequence gaps, changing edge clamping, or
publishing semantics before their pixels.

This is a user-space input adaptation decision. The kernel continues delivering
the same ordered, copied physical records, and human and machine clients keep
the same granted semantic commands. The optimization must not turn the
physical-input queue into an authenticated semantic source or extend the ABI.

## Decision

Add bounded ready-motion consumption to the workspace pointer adapter. One
canonical update may incorporate at most 16 already-ready continuous pointer
packets, including the first packet. The adapter polls for additional input
without waiting. An empty source, the packet limit, or a semantic boundary ends
the update immediately; no timer or deliberate delay collects more input.

Only continuous motion from the same valid source generation with the same
button mask is eligible. The first packet requires an already-established
generation and prior sequence, with no post-loss sequence-zero requirement
pending. Fresh sources and transitions remain singleton updates and do not
poll. A steady held chord may batch motion while its capture stays cancelled.
Entering or leaving that chord remains a boundary. Validate every incorporated
packet and its exact raw sequence before advancing the adapter. Do not manufacture a replacement raw
packet, conceal missing sequences, renumber events, or treat the batch as an
exception to generation and loss checks.

Apply each packet's relative motion in order and clamp to the viewport after
each step. Summing deltas and clamping once is forbidden: a move beyond an edge
followed by movement away from it has a different result. Retain the existing
gesture's original capture anchor and window incarnation for drag or resize;
the intermediate packets do not create new presses, move the capture anchor,
or issue separately published geometry transactions.

Press, release, button-chord changes, loss, keyboard input, and source changes
remain separate observations. A first packet that represents a boundary uses
the ordinary single-event path. While draining eligible motion, retain at most
one fetched boundary record as untouched lookahead for the next loop turn.
Do not apply it to the pointer adapter, convert it into motion, consume its
transition, or read beyond it. It is handled next through its usual validation
and policy path, preserving input order and cancellation behavior.

At the 16-packet cap, no additional lookahead is fetched: a full batch performs
at most 15 further polls after its first packet.

Prepare adapter changes in a local copy. If validation or the input reader
fails, leave the original pointer state unchanged and withdraw the native
owner; already-read input must not be retried as though it were still queued.

Project the resulting pointer position and current gesture into one existing
bounded semantic command batch. Canonical preparation still requires the same
observation, arrange/manage/customize grants, exact committed revision, and
model validation. It reserves one successor and receives one complete-frame
acknowledgement before publication. Any armed control is continued only under
the ordinary original-target and revision checks; batching gives it no extra
authority. Expected policy rejection and unexpected failure retain their
existing recovery and withdrawal behavior.

The bound changes presentation cadence for continuous physical motion. It does
not promise a frame for every raw motion packet. Public human, accessibility,
and machine views still observe the same canonical revision and pixels; an
unpublished intermediate pointer position is not separately visible to any
client. Button and keyboard semantics remain unchanged.

## Alternatives and consequences

Retaining one frame per packet has the simplest projection but can preserve a
backlog after motion stops. Increasing pointer sensitivity changes displacement
and does not reduce that backlog. Unbounded draining risks delaying other input
and presentation indefinitely. Delaying to form batches adds latency when the
queue is empty. Combining only accumulated deltas breaks viewport-edge
semantics. Partial redraws, a hardware cursor, or kernel-side coalescing would
require broader presentation or input decisions and are outside this slice.

The bounded approach adds one lookahead record and bounded adapter work while
reducing redundant full frames when motion is already queued. Intermediate
hover paint can be omitted within a continuous batch; the final canonical
state remains shared. Isolated input should continue to publish immediately.
Throughput and latency remain sensitive to the host, backend, theme, and load;
this decision makes no real-time or physical scanout guarantee.

## Validation, disproof, and reversal

Require adapter tests for the 16-packet limit, no polling beyond a boundary,
single untouched lookahead, every sequence/generation check, edge reversal with
sequential clamping, source/loss/chord/keyboard boundaries, and unchanged
release activation. Exercise held drag and resize against the original capture
anchor. Verify that one batch reserves one canonical revision, and rejected
updates preserve the existing rollback and cancellation behavior.

Preserve the paced native window, Appearance, and release-lifecycle proof, and
add queued-input evidence with independently chosen semantic expectations.
Require complete frames, final cursor/geometry and button state, strict source
handling, containment, reclamation, normal-image exclusion, and the complete
repository validation matrix. Measure sustained motion under comparable host
conditions with acknowledged injection and concurrent serial observation;
separate queue catch-up from host monitor and scanout effects.

A missed button/key boundary, accepted sequence gap, incorrect edge position,
changed capture anchor, extra canonical revision, premature publication, or
failure to improve bounded queue catch-up would disprove the implementation.
Reversal restores the prior single-event adapter call in the native loop and
removes the optional lookahead without changing kernel records, wire formats,
grants, or the independently useful rendering and transfer optimizations.
Record measured results in the notebook separately from this proposal.
