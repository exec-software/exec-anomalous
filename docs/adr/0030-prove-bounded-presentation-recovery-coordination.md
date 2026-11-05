# ADR 0030: Prove bounded presentation recovery coordination

- Status: Proposed
- Date: 2026-09-26

## Context

ADR 0029 proves one complete native rendering and framebuffer copy followed by
publication in a retained host semantic transaction. Its source, renderer and
supervisor have all been destroyed before the copy. A host failure between
the copy and publication can leave successor pixels beside old semantics.
Adding a receipt-shaped reply does not resolve this interval: receipt bytes
are not authority, and a lost reply must not lose the transaction's outcome.

A native service also needs facilities outside the existing proof: private
writable service memory, a retained service incarnation, authenticated
cross-process operations, appropriate supervision capacity, and a renderer
launch contract that accepts fresh generations. Combining all those changes
with a new recovery policy would obscure which invariant each proves.

This decision first defines a bounded, executable **host-only prerequisite**
using the real ADR 0027 service. It does not accept a native service, change the
kernel, or claim to close ADR 0029's native crash gap.

## Decision

Add the allocation-free, `no_std`, unsafe-free
`anomalous-desktop-recovery` crate. One coordinator privately owns one actual
`desktop_surface`, its surface and presentation controls, its renderer
endpoint, and all pending progression owners. It exposes no underlying service
or control accessor that could bypass its observation gate.

The constructor inherits ADR 0027's trusted assertion that the initial semantic
revision is already presented. This component neither renders nor verifies
that initial frame. A native proof will need its own initial-frame evidence.

### Separate copy, semantic publication and availability

The lifecycle separates the following transitions:

1. Prepare and render one successor under the existing strict backpressure.
   Readers may still borrow the complete committed scene during this work.
2. Begin presentation, retaining the exact ready owner and redacted snapshot
   and closing the public semantic-read gate **before** any external write.
3. Record the trusted presenter's assertion that the complete matching frame
   has been copied. A partial copy, dropped request, failure or lost response
   supplies no such assertion.
4. Consume the retained ADR 0027 progression owners to publish the semantic
   revision exactly once. Keep external semantic reads unavailable.
5. Finish the exact presentation transaction and reopen the complete paired
   human/accessibility/machine view. Only then release recovery data.

`unavailable` is an explicit result, not a fallback to old semantics or an
empty scene. The coordinator offers no action or focus execution path. Future
facades must obey the same gate. Borrowed views cannot coexist with a mutable
presentation transition; copied observations outside this boundary remain
historical data and cannot be revoked by this component.

The interval is recoverable unavailability under a serialized software model,
not simultaneous pixel/semantic visibility or hardware atomicity. A person
may see partial or new pixels while semantic reads are unavailable.

### Keep authority local and outcome queries repeatable

Presenter endpoints, recovery controls and presentation tickets are opaque,
move-only, privately branded local owners. Endpoint generation and exact
transaction freshness are checked on every progression call. Equal public
surface, scene, renderer, revision or attempt values do not make an owner from
another coordinator valid. Freshness exhaustion fails closed without wrapping.

Only the holder of the separate recovery control may replace a presenter.
The presenter cannot create a replacement by submitting identifying bytes.
Dropping an endpoint or ticket does not advance publication or reopen reads.
Status queries describe retained state without consuming it or triggering a
copy, publication or finalization.

These are local ownership boundaries. No endpoint or ticket is serialized;
this crate authenticates neither an operating-system process nor a remote
party. The adapter holding the presenter endpoint is trusted to report a
complete matching copy accurately. A dishonest assertion remains outside this
proof, just as it does in ADR 0027.

### Recover forward through one fresh presenter

Permit one controller-authorized presenter replacement per coordinator
lifetime, during an active presentation only. The trusted adapter must stop
the old external presenter before replacement; local ticket invalidation
cannot revoke device access or prevent a concurrent old writer. Invalidate
all old presenter endpoints and tickets and preserve the
exact pending transaction and redacted drawing body. Never reopen the previous
semantic revision merely because the presenter failed: pixels may already
have changed.

The new presenter must redraw the entire retained frame and assert completion
under a fresh ticket before progress resumes. This applies even when semantics
were already published but final acknowledgement was lost. In that case, do
not publish again or increment the semantic publication sequence twice; only
restore the matching frame and finish the retained transaction.

Replacement is not renderer restart. It does not re-encode the body, alter its
renderer attempt, resubmit the scene candidate or spend ADR 0027's separate
renderer-replacement authority. The retained body describes the same frame.

All rejections preserve the state and every private owner required to continue
or retire. Retirement is explicit and possible while unavailable. Retirement
withdraws local semantics and drops private owners; it does not erase a display
or claim that old pixels disappear.

### Bound privacy and trust claims

Recovery retains only the existing redacted transport body in addition to the
service's existing private canonical owners. Protected values and lengths must
not affect recovery bodies, status or error shapes. Errors remain static codes;
owners have no implicit formatting, hashing or byte-conversion surface.

Operation count, phase, revision and recovery activity are observable. Logical
clearing and owner lifetime are not secure erasure or timing noninterference.
The coordinator must survive for recovery to work. Losing the coordinator,
its private canonical state, the host process or power is not a supported
recovery event. Nor does this model recover an idle display failure after the
transaction and its redraw body have already been finalized.

## Validation required for the host prerequisite

- Exercise the real ADR 0027 service and ADR 0028 encoder, not a duplicate
  semantic-state implementation.
- Interrupt before copy, during a simulated partial copy, after complete copy,
  after copy acknowledgement, after semantic publication and before finish.
  Observe unavailability until the exact matching full frame is restored.
- Recover each interruption with one fresh presenter; reject stale and foreign
  endpoints, tickets and controls, including equal public identities.
- Check strict candidate return/backpressure, no second publication, read-only
  outcome queries, owner-drop non-transitions and retirement from every phase.
- Check protected inputs of unequal lengths, compile-time owner privacy and
  freshness exhaustion. Run the repository's existing host and QEMU regression
  matrix; those QEMU runs remain regressions, not native evidence for this crate.

## Host implementation evidence

The host implementation passed 5 unit, 12 integration and 37 compile-fail tests,
including counter exhaustion, all 64 valid/32 invalid scene states, phase
rejections, foreign and stale owners, six simulated interruption cuts and
redacted body/frame equality. `make check`, the complete serial QEMU regression
matrix and final normal-image exclusion also passed. See the
[development log](../../notebook/Development/2026-09-26%20Presentation%20Recovery%20Coordination%20Log.md)
for the evidence scope. These are host-contract and existing native regression
results, not deployment evidence; this ADR remains Proposed.

## Native follow-on

A later native decision must bind presentation operations to current task,
channel, endpoint and supervision state, preserve outcomes across failed reply
copies, and keep the semantic service alive through final acknowledgement.
It must define private RW+NX service data separately from the RX image, handle
the required supervision relationships, and verify fresh renderer bindings.

The native service-failure policy must explicitly choose forward recovery from
a surviving canonical owner, withdrawal of the visible surface, or rollback to
a previously verified frame and canonical checkpoint. No existing canary
baseline counts as such a checkpoint. Protected canonical content will reside
in the service's private native memory; renderer, transport, telemetry and
capture redaction must remain separate from that ownership.

## Alternatives and consequences

Publishing immediately after render completion was rejected because rendering
does not establish completed presentation. Returning old semantics while the
copy is uncertain was rejected because it advertises potentially incorrect
alignment. Treating public receipts as capabilities was rejected because
copyable fields do not establish identity or authority.

Rollback is deferred: it needs both a verified old frame and a surviving
canonical checkpoint. Forward redraw uses the exact already-retained successor
and leaves uncertain reads explicitly unavailable. This costs temporary
availability and a full redraw, but keeps recovery and evidence bounded.

The host component is intentionally separate from the kernel and native proof.
It introduces no wire format, syscall, new privilege boundary, stable ABI,
window lifecycle, compositor, scheduler, input policy or normal-image desktop.
