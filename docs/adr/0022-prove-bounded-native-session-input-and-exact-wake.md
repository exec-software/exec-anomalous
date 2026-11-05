# ADR 0022: Prove bounded native session input and exact wake

- Status: Accepted
- Date: 2026-09-13
- Accepted: 2026-09-19

This record accepts one removable x86-64 proof that carries a versioned,
bounded semantic input delivery across the native rite boundary, blocks one
exact consumer without losing a publication, and makes only that blocked task
runnable after a complete private publication. The selected consumer then
acknowledges that exact wait and copies the reply before source consumption. It
does not define a production input router, focus service, secure-attention
path, shell language, stable syscall ABI, or general scheduler policy.

## Context

ADR 0013 keeps physical scan bytes and key transitions private to the kernel
input mechanism. Human or machine authority must never be inferred from that
device stream. ADR 0019 defines the current fixed-width native rite prefixes,
but its `input_read` operation has a zero-byte request and can return only an
empty `would_block` reply. ADR 0020 composes that result with a supervised
shell-shaped vessel. ADR 0021 then proves, above the kernel boundary, bounded
private semantic actions and a lost-wake-safe session state machine.

The remaining boundary is privileged and cannot be filled by simply exposing
the PS/2 queue or retaining the current rite admission across a task switch:

- the physical `input_read` right describes kernel input-familiar access, not
  routed shell-session semantics;
- the rite admission exclusively borrows its channel, fails closed if
  abandoned, and currently has no owned pending-read form;
- the one-page x86-64 transport writes only the 40-byte reply prefix and has no
  accepted reply-body transaction;
- the scheduler can block a task, but its ordinary wake operations are coupled
  to another task's terminal supervision transition; and
- scheduler identity alone cannot distinguish a session-input wait from a
  supervision wait or a later wait by the same task incarnation.

A naïve implementation creates several security failures. Consuming a queue
entry before the complete user copy loses private input on copy failure.
Publishing a success marker before its body permits a torn reply. Checking an
empty queue and then sleeping can lose an intervening publication. Waking by
task identity alone can resume the wrong wait. Letting an origin enum select
authority allows a machine client to claim human provenance. Failing to clear
queued typeahead on route closure can carry commands across focus or session
replacement.

The smallest honest next step is therefore a feature-only mechanism proof with
a kernel-authored deterministic publisher. It must exercise the native copy,
pending-call, readiness, scheduler-placement, interruption, and teardown
transactions without claiming that the fixture authenticates a real source or
decides focus policy.

## Decision

### Keep physical and semantic input separate

Add a new `session_input_read` rite right and operation. Do not redefine the
existing physical `input_read` operation, consume the global PS/2 event queue,
or add scan codes, key transitions, device identifiers, or controller timing to
the new payload.

The current `shell_minimum` and `display_manager_minimum` grants remain
unchanged evidence from ADR 0019. The new proof grants only the exact rights its
fixed consumer uses. A future production shell grant may replace physical
`input_read` with `session_input_read` only after an authenticated input
familiar and session host exist.

Kernel-authored human and machine origin fixtures use the same endpoint, queue,
request, reply, copy-out, wait, and wake mechanism under the consumer's same
exact rite grant. Their origin classes remain distinct private attribution.
This proves delivery-mechanism equality, not client authentication or routing
authorization. No origin class, action tag, identity value, trace, reference,
payload byte, or task address confers authority.

### Versioned fixed wire bodies

Assign right bit `1 << 5` and operation value 6 to `session_input_read`. Assign
payload-kind value 6 to `session_input_request` and value 7 to
`session_input_delivery`. Extend `anomalous-abi` with transparent `u16`
wrappers for semantic action and origin tags plus two exact version-1 bodies:

- `rite_session_input_request_v1` is 16 bytes. Offset 0 is a `u16` structure
  size of 16; offset 2 is version `u16` value 1; offset 4 is a `u32` flags word;
  offset 8 is a `u32` reply capacity of 280; and offset 12 is a zero `u32`
  reserved field. Flag bit `1 << 0` is `wait_if_empty`; every other bit is
  reserved and rejected.
- `rite_session_input_delivery_v1` is 280 bytes. Offsets 0 and 2 are its `u16`
  structure size of 280 and version value 1. Offsets 4 and 6 are `u16` action
  and origin tags. Offsets 8 and 10 are `u16` text-byte length and erase-scalar
  count. Offset 12 is a zero `u32` reserved field, offset 16 is one nonzero
  `u64` delivery sequence, and offset 24 begins the 256-byte text area.

Action values are `insert_text=1`, `erase_previous=2`, `submit=3`,
`cancel_submission=4`, and `source_discontinuity=5`. Origin values are
`human_text=1`, `machine_text=2`, `paste=3`, `accessibility=4`, and
`remote_text=5`. Unknown values remain representable data and are rejected;
Rust never decodes arbitrary bytes directly into an enum discriminant.

Both bodies use explicit little-endian transport encoding. The request remains
a body following `rite_call`; `rite_call::payload_limit` remains a request-body
bound rather than being silently reinterpreted as response capacity. Version 1
requires `operation=session_input_read`,
`payload_kind=session_input_request`, `payload_size=16`, and
`payload_limit=16`, plus a declared reply capacity equal to the complete fixed
delivery size. Unknown operation remains `bad_operation`; a missing required
right remains `denied`; outer arithmetic or `payload_size > payload_limit`
remains `limit_exceeded`; and wrong kind, size, larger limit, body version,
flags, reply capacity, or reserved value returns `bad_payload`. That exact rule
is removable pre-alpha evidence, not a promise that every later version has the
same size.

The delivery body is always its complete fixed size, so the outer reply does
not disclose the exact text length. `insert_text` requires 1 through 256 valid
UTF-8 bytes, a zero erase count, and a zero unused tail. Version 1 inherits ADR
0021's exact scalar policy: it accepts Unicode scalar values plus literal
U+0009 tab and U+000A line feed, rejects every other Unicode control scalar,
and also rejects U+00AD, U+034F, U+061C, U+180E, U+200B through U+200F,
U+202A through U+202E, U+2060 through U+206F, and U+FEFF.
`erase_previous` requires a zero text length, a count from 1 through 256, and an
all-zero text area. `submit`, `cancel_submission`, and
`source_discontinuity` require zero length, zero count, and an all-zero text
area. Paste attribution is valid only for `insert_text`; a pasted line feed
remains data and never becomes `submit`. Unknown versions, flags, tags,
reserved values, or invalid combinations fail before disclosure or mutation.

An accepted delivery returns `status=ok`, payload kind
`session_input_delivery`, and payload size 280. An empty request without
`wait_if_empty` returns the existing payload-free `would_block` status. Route
closure of a blocked request and an otherwise authorized read after closure
return payload-free `interrupted`. Every response retains the original rite
sequence and trace in the private task mailbox; those values remain forbidden
from telemetry.

### Fixed private endpoint and exact identities

Add an architecture-neutral, allocation-free session-input endpoint compiled
only for host tests and the new proof feature. It owns four fixed private FIFO
slots, one exact consumer task incarnation, at most one wait, and checked
nonzero identities for:

- endpoint generation;
- session generation;
- consumer generation;
- route epoch;
- delivery sequence;
- wait sequence; and
- task-local readiness generation; plus
- endpoint revision.

The endpoint and every wait bind the complete native `task_identity`, including
task and circle generations. Delivery, wait, readiness, and revision cursors
start at one and never wrap to zero. A successful publication consumes one
delivery sequence and advances revision once. When a wait is live, publication
must leave four further revision values available. Installing a wait consumes
one wait sequence and one readiness generation and advances revision once.
Immediate receive advances revision once. Selected completion of an
input-ready wait acknowledges that wait and consumes its head as one native
atomic transition that advances revision twice. This corresponds to ADR 0021's
separate acknowledgement and receive transitions without exposing an
intermediate native state. First route closure advances revision once;
completing its terminal pending read advances revision once. Preparation,
scheduler preflight, repeated closure, and every rejected transition consume no
endpoint counter and change no endpoint state.

Every ordinary non-wait mutation reserves two further revision values for
route closure plus terminal pending-read completion. Closure without a wait
requires one value; closure with a wait requires two, and the terminal
completion may consume the last. The last complete wait, publication, selected
delivery, and retirement frontier starts at revision `u64::MAX - 6`; starting
at `u64::MAX - 5` makes the later live-wait publication reject without
mutation. Wait installation also proves that the admitted task accounts for
one future runnable slot and that the current dispatch sequence can be reserved
until the next exact selection. No unrelated scheduler transition may consume
that reservation. Exhaustion is explicit and failure-atomic; it cannot strand
an accepted private entry or pending read.

The endpoint state machine is exactly `open_idle`, `open_blocked`,
`open_input_ready`, `closed_idle`, or `closed_terminal`. A prepared operation is
move-only read state, not an installed state. `open_input_ready` and
`closed_terminal` retain the same exact wait/readiness tuple until the selected
consumer completes it once. Numeric identity equality establishes freshness
only. The proof's kernel-authored publisher already holds the test endpoint and
supplies fixed validated actions; it is not a public publish rite, an
authentication service, or evidence that a real person, device, agent, paste
source, accessibility service, or remote peer was verified.

This feature-only endpoint does not promote the entire ADR 0021 shell-session
owner into the normal kernel. To prevent a second model from drifting silently,
host conformance tests replay the same bounded action, origin, FIFO,
prepare/commit, input-ready, closure, replay, and exhaustion traces through the
ADR 0021 model and the native endpoint and require equivalent accepted or
rejected outcomes at their shared boundary.

The decoded delivery body and private queue owners are move-only and implement
none of `Clone`, `Copy`, `Debug`, `Display`, `Hash`, or general serialization.
The fixed little-endian reply encoder is a named local disclosure boundary to
the exact consumer mailbox, not an implicit serialization facility. Rejection
returns private ownership rather than dropping it silently. Consumption, route
closure, teardown, and `Drop` share one logical clear path for text, metadata,
and unused tails. Ordinary stores may still be optimized or copied, so this is
not secure erasure of caller copies, registers, caches, speculation, swap, or
hardware state.

### Lost-wake-safe blocking and owned Rite correlation

Session reads use an explicit prepare/commit protocol under the same serialized
runtime exclusion as publication:

1. Admission validates the exact running task, grant, operation, request-body
   shape, transport spans, and reply capacity before interpreting session
   state.
2. Receive preparation captures the exact endpoint, session, consumer, route,
   task, proposed wait sequence, readiness generation, and endpoint revision.
3. Commit revalidates every value. A nonempty queue dominates a changed
   revision and forces immediate delivery; the task must not block. Only the
   same empty revision may install one exact wait.
4. Before selecting another task, the synchronous `rite_admission` is consumed
   into pending state inside the exact channel. That state owns the operation,
   task/circle, endpoint/session/consumer/route, wait/readiness, rite sequence
   and trace, reply capacity, and a generational task-owned mailbox mapping
   identity. It stores no raw user pointer or Rust reference.
5. A scheduler preparation reserves the exact future dispatch sequence and
   proves both the current block/peer dispatch and eventual consumer return.
   The endpoint wait, task-local readiness record, channel pending state, and
   dispatch reservation are committed with the scheduler's running-to-blocked
   transition as one prepared, infallible composite.

The channel state is exactly `idle`, synchronously `admitted`,
`session_input_pending`, or `closed`. It refuses every new admission while a
session read is pending. A pending call can complete only through the same live
channel and full exact correlation; retirement first requires input-ready or
terminal completion. Channel recreation for a live pending task is forbidden.

The stable outer proof runtime owns the endpoint, channel, mailbox mapping
lease, pending state, and saved kernel continuation. Destructor-bearing owners
may persist there. The prohibition applies to stack-local gate, interrupt, and
exception frames that will be abandoned by selected-frame return: no
`rite_admission`, live exclusive borrow, temporary private owner, uncommitted
prepared token, or destructor-dependent cleanup may remain on such a frame.
The selected consumer later rederives its fixed virtual mailbox through the
exact task-owned address-space and mapping identities and revalidates RW+NX
ownership before copying.

Input arriving before wait commit prevents sleep. Input arriving after commit
is installed completely before the wait becomes input-ready. Publication does
not copy or consume the head and does not complete the pending rite. Equal
copies of a wait or readiness identity confer no authority and cannot complete
the current wait more than once.

### Exact blocked-to-runnable placement

Add one endpoint-gated scheduler composite that consumes a move-only prepared
wake containing the exact endpoint, session, consumer, route, task/circle,
wait, and readiness identities. It changes that exact admitted task from
`blocked` to `suspended` and appends it once to the runnable FIFO while leaving
the current task alive and running. There is no crate-visible unguarded
`make_runnable(task_identity)` surface. The scheduler must not make `blocked`
generally dispatchable or reuse the terminal `fault_current_and_wake` or
`complete_current_and_wake` semantics.

The endpoint remains the event authority; the scheduler supplies placement
only. Before any input-ready or terminal wake commits, the scheduler must
preflight:

- exact task and circle incarnation;
- admitted saved context;
- `blocked` task state and absence from the current slot and runnable queue;
- runnable-tail capacity;
- unchanged current task and scheduler structure; and
- the exact reserved dispatch sequence for this wait remains live.

After that preflight, one infallible scheduler commit performs the task table's
exact `blocked -> suspended` transition and FIFO-tail insertion without an
intervening observable or fallible step. A session wake cannot wake a task
blocked on supervision, a different endpoint, another session or route, an old
readiness generation, a replaced task, or a later wait. Duplicate wake attempts
are failure-atomic and cannot enqueue the task twice.

Because scheduler capacity already includes every admitted task, a blocked
admitted task accounts for the slot needed to return it to the runnable set.
Host tests must prove that invariant under full-capacity and wrapped-queue
states rather than relying on it implicitly.

The fixed scheduler stores at most one outstanding move-only dispatch
reservation in this proof. It removes that sequence from the ordinary cursor at
wait installation; unrelated dispatch and preemption paths cannot consume it.
Only a prepared transition selecting the exact reserved consumer may use it,
and completion, route closure, or teardown must consume or retire it exactly
once. A copied reservation is unavailable by construction.

### Wake acknowledgement, copy before consume, and publish last

Publication to an installed wait first enqueues the complete private action,
then changes the endpoint from `open_blocked` to `open_input_ready`, then commits
the endpoint-gated `blocked -> suspended` FIFO-tail wake. It does not write the
mailbox, consume input, clear the wait, or settle the channel. A route close may
therefore replace `open_input_ready` with `closed_terminal` and clear typeahead
until the exact consumer is actually selected.

The selected-frame path is the acknowledgement. Before a scheduler transition
selects the runnable consumer, the stable runtime preflights the exact pending
wait/readiness/channel correlation, reserved dispatch sequence, endpoint
outcome, mailbox mapping, reply span, and source head. The scheduler and
transport then use this bounded order under one exclusion:

1. encode either the complete fixed delivery or payload-free interruption into
   zeroed kernel scratch;
2. invalidate the old reply header and clear the complete 280-byte reply area;
3. copy the delivery body when input remains authoritative and write every
   reply-prefix field except the header-size marker;
4. commit the prepared scheduler transition selecting the exact consumer with
   its reserved dispatch sequence;
5. derive the acknowledgement from that now-current exact task, complete the
   matching input-ready or terminal wait once, consume and logically clear an
   input head only after its complete body copy, and settle the pending channel;
6. publish the reply header size last; and
7. return only the fully validated consumer frame.

The exact same-mailbox request/reply area at offset 40 is a permitted temporal
overwrite only after the complete request and correlation have been copied into
kernel-owned state. The consumer's one task-owned RW+NX user mapping and the
kernel's supervisor-only physical alias are the two deliberate views used for
copying. Partial, cross-page, foreign-mailbox, stale-mapping, or any additional
user-writable alias is rejected.

All fallible native validation and encoding precedes step 2. Steps 2 through 7
are bounded and infallible under the proof's single-BSP, IF-clear, NMI-masked
exclusion; a hardware fault in the fixed mapped mailbox is fail-stop rather
than a recoverable short copy. Separately, a host copy model injects failure at
every body byte before the native commit. It permits an invalid or partial
destination only with the header invalid and proves that source queue, wait,
counters, complete pending channel, task table, scheduler, and retained
selection authority remain unchanged.

Poll-mode immediate delivery uses the same validation, invalidation, body copy,
source commit, and header-last publication but creates no wait or scheduler
transition. Long-text followed by short-text delivery and interruption clear
scratch, the full body area, and unused tails so prior private bytes cannot
survive in a later result.

### Route closure and externally decided focus loss

The endpoint exposes a kernel-internal exact route-closure transition. The proof
fixture may invoke it with `focus_lost` as a reason, but the kernel does not
discover focus, authenticate the caller, or decide focus policy.

Closure serializes with publication, wake, acknowledgement, and delivery. It
first preflights revision headroom, the exact pending channel/wait correlation,
the interruption reply span and mailbox mapping, and any scheduler placement
needed for a still-blocked task. The remaining close commit is infallible: mark
the route closed, logically clear all queued typeahead, replace
`open_input_ready` with `closed_terminal` when necessary, record a payload-free
interrupted outcome for the pending call, and wake a still-blocked task exactly
once. An already-runnable waiter is not enqueued again.

A repeated closure is idempotent and cannot create another wake. An authorized
read arriving after closure returns payload-free `interrupted`; an unauthorized
caller still receives the uniform denial before endpoint state is inspected.
Delivery and closure are totally ordered by reply-header publication: delivery
that reaches that marker is owned by the selected consumer and later closure
cannot retract or securely erase it. Closure winning before the selected-frame
copy instead clears the queued entry and makes the exact pending call terminal.
Tests must exercise both complete orders and reject every partial hybrid.

No same-lineage endpoint replacement is required in this slice. Orderly
teardown closes the route, selects the exact consumer if a terminal pending read
remains, publishes and acknowledges its one interrupted completion, retires the
idle channel, clears all endpoint and transport storage, and only then
withdraws task and address-space storage. Failure to complete this order is a
terminal proof failure, not permission to discard a waiter silently.

### Removable x86-64 proof

Add a separate `input_wait_test` feature and `make input-wait-smoke` target. Do
not alter the closed-world traces accepted for `rite_test` or
`shell_vessel_test`. The new feature has highest precedence in all-features
test builds and reuses the accepted 22-qword scheduler frame, private task
address spaces, privilege stacks, fixed one-page mailbox, and safe original
kernel-stack checkpoint.

The deterministic proof uses one exact ring-3 consumer and one exact peer under
a kernel-authored test route:

1. A fixed human-attributed semantic action is queued before dispatch. The
   consumer reads and validates the complete nonempty reply through the native
   gate, proving immediate copy-before-consume.
2. The consumer issues an empty wait request. The kernel converts its admission
   to owned pending state, blocks it, and dispatches the peer.
3. The peer reaches a validated test trigger. The kernel publishes a fixed
   machine-attributed action with the same semantic shape, marks the exact wait
   input-ready, and appends only the consumer to the runnable tail while the
   peer remains running. It neither copies nor consumes the action.
4. The peer performs a prepared ordinary preemption that consumes the wait's
   reserved dispatch sequence and selects the consumer. The selected-frame path
   acknowledges the exact wait, copies before consuming, and publishes the
   reply last. The consumer validates the complete body, distinct attribution,
   equal semantic shape, preserved rite correlation, and one delivery only;
   the peer remains suspended at the runnable tail.
5. The consumer waits again. After the peer resumes, the fixture closes the
   exact route as an externally decided focus loss. The kernel clears queued
   state, records a terminal pending outcome, and performs one exact wake. The
   peer then executes one exact contained `#UD`; a prepared fault transition
   terminalizes the peer and selects the consumer with the reserved dispatch
   sequence. The selected-frame path acknowledges the terminal wait and
   publishes the payload-free interruption.
6. The consumer validates that interruption and completes with no runnable
   peer. Before the nonlocal kernel resume, the proof masks delivery, restores
   kernel CR3 and bootstrap TSS RSP0, validates and exclusively consumes the
   saved kernel continuation, and clears every active/switch/gate publication.
   Only after return on the original kernel stack may it retire the idle
   channel and closed endpoint, withdraw and clear task roots, user pages,
   privilege stacks, and transition stacks, restore all accounting, emit the
   deferred transcript, and exit with the established success status.

The transcript is closed-world, structured, and content-free. It reports fixed
lifecycle classes and booleans such as `body_copied=true`,
`source_consumed=true`, `content_redacted=true`, and `exact_wake=true`. It
contains no input text, action or origin tag, exact text length or erase count,
queue depth, endpoint/session/consumer/route/wait/readiness/delivery identity,
rite trace or sequence, pointer, register, root, physical address, canary,
fixture marker, or timestamp. Event shape does not authorize a production sink
or retention policy.

The ordinary image must contain none of the feature namespace, gate wiring,
publisher fixture, route labels, private bodies, markers, or feature-only
helpers. The proof remains x86-64 native. IA-32 execution and 32-bit Win32 ABI
translation remain future user-space compatibility-veil work that validates and
copies foreign values before invoking native rites.

## Required invariants

- Physical input and semantic session input remain different rights,
  operations, payloads, queues, and authority domains.
- Human and machine semantic input use one mechanism while retaining distinct
  private attribution; neither can fabricate physical provenance.
- Every accepted request, action, wait, pending rite, wake, and task placement
  binds the exact live generational identities and never wraps a counter.
- IDs, origins, action tags, traces, references, mailbox contents, and task
  addresses confer no authority.
- Unknown or unauthorized requests fail before private route state or payload
  inspection and disclose only a uniform bounded status.
- No accepted action is overwritten, silently dropped, duplicated, reordered,
  or consumed before the complete destination body copy.
- No valid success marker is visible before a complete body and committed
  source transition; interruption carries no stale body.
- Input before block prevents sleeping. Input after block is completely queued
  before the exact task becomes runnable and remains unconsumed until the
  selected consumer acknowledges it.
- A task is enqueued at most once for one exact wait, at the FIFO tail, without
  changing the still-running publisher.
- No synchronous rite admission, unsafe user pointer, or exclusive borrow
  survives a task switch.
- Route closure clears queued typeahead, converts an input-ready wait to the
  same terminal identity, and resolves an exact pending read once; it cannot
  retract an already linearized consumer copy.
- Teardown retires the endpoint and channel and clears owned private storage
  before task or mapping reuse.
- Privacy-safe evidence authorizes no telemetry sink, persistence, history, or
  audit retention.

## Consequences

This proof establishes the first nonempty native semantic input response and
the first nonterminal exact scheduler wake. It also makes the current
shell path more honest: a future shell vessel can receive routed semantic
actions without reading a global physical-key stream, and human and machine
clients can share one low-level path without erasing attribution.

The cost is a new experimental right, operation, and wire body plus a sizable
feature harness. The ABI remains explicitly pre-alpha and removable. The
feature does not make the normal image interactive and does not move router or
focus policy into the kernel.

## Alternatives considered

### Redefine `input_read` as semantic input

Rejected. It would collapse the physical input familiar and a private session
route into one authority surface, contradicting ADR 0013 and making display,
shell, and AI consumers harder to reason about.

### Expose the PS/2 event queue to the shell

Rejected. A global physical stream would cross focus and session boundaries and
would let software confuse device provenance with human authority.

### Keep returning `would_block` and poll

Rejected as the next proof. Polling avoids the wait transaction rather than
proving it, wastes execution, and leaves the lost-wake and exact-readiness
boundary unresolved.

### Hold `rite_admission` across scheduling

Rejected. Its exclusive borrow and fail-closed destructor belong to one
synchronous kernel frame and cannot safely survive nonlocal frame selection.

### Wake by task identity alone

Rejected. The same task incarnation can block for different reasons and can
perform successive waits. Endpoint, route, wait, and readiness generations are
required to prevent confused wakeups and replay.

### Put authentication and focus policy in this slice

Rejected. Neither current code nor a deterministic test fixture can establish
a real human, device, agent, seat, or remote principal. Those services should
remain replaceable policy outside this bounded kernel mechanism.

## Non-goals

This decision does not define or implement:

- deployed source authentication, secure attention, focus arbitration,
  multi-seat policy, trusted paste, accessibility trust, remote trust, or a
  physical-input familiar;
- a public user-space publisher, general IPC route, sigil distribution scheme,
  global identity allocator, or production session host;
- Canticle grammar, parsing, formatting, ASTs, evaluation, completion, history,
  clipboard, secret entry, rendering, or command effects;
- multiple endpoints, consumers, waiters, broadcast, fan-out, arbitrary user
  pointers, general user-copy recovery, signals, deadlines, timeouts, fairness,
  priorities, or cancellation policy;
- stable rite, session, task, executable, package, shell, or compatibility ABI;
- SMP, APIC, NMI enablement, hostile concurrent mailbox writers, DMA/IOMMU
  isolation, side-channel resistance, secure erasure, crash persistence, or
  production confidentiality; or
- an IA-32 kernel. x86 compatibility remains a user-space veil below the native
  x86-64 control surface.

## Acceptance requirements

Host tests must cover:

- exact request and delivery sizes, offsets, little-endian encoding, known
  numeric version/right/operation/payload/tag/flag values, zero reserved fields,
  exact reply capacity, and every valid and invalid action/origin combination;
- fixed body size, exhaustive ADR 0021 scalar-policy conformance, UTF-8/text
  bounds, zero tails, paste/newline separation, move-only private ownership,
  logical clearing, compile-fail trait checks, and non-formatting errors;
- immediate delivery, empty poll, prepare/commit, publication before prepare,
  between prepare and commit, and after block;
- FIFO wrap/straddle, full backpressure, returned ownership, duplicate and
  replayed work, and sequence/revision/readiness exhaustion with unchanged
  state;
- bounded trace conformance between the ADR 0021 host model and the native
  endpoint for every shared action, origin, queue, wait, wake, closure, replay,
  and exhaustion outcome;
- wrong and stale endpoint, session, consumer, route, task/circle, wait,
  readiness, rite, and scheduler identities independently;
- wake of a supervision wait, another task, current/runnable/terminal task, and
  a later wait, plus full scheduler capacity, FIFO-tail placement, duplicate
  wake, unchanged-current behavior, exact dispatch reservation, reservation
  theft/replay, and maximum-sequence retirement without a stranded waiter;
- undersized, overflowing, unmapped, wrong-version, bad-reserved, malformed,
  cross-page, foreign-mailbox, additional user-writable alias, and stale
  request/reply spans, while permitting the exact task mapping plus
  supervisor-only kernel alias and only the fully staged temporal overwrite of
  offset 40 in that mailbox;
- host-model injected failure at every delivery-copy byte before native commit,
  proving an invalid success marker and unchanged
  endpoint/private queue, wait/counters, complete pending channel, task table,
  scheduler, and retained selection authority;
- long-text delivery followed by short-text delivery and interruption, proving
  complete scratch, mailbox-tail, queue, and teardown clearing;
- route closure while empty, queued, blocked, already closed, and on both sides
  of the successful-copy linearization point; and
- exhaustive bounded transition traces for publish, prepare, commit, deliver,
  close, wake, dispatch, retry, and teardown.

The x86-64 proof must validate the fixed immediate, blocked-publication, exact
wake, resumption, and route-closure interruption lifecycle with a strict
content-free transcript. Mutation tests must reject reordering, duplication,
missing lifecycle records, private or correlating fields, malformed structured
output, forbidden content in complete or split form, and altered restoration
accounting.

Acceptance also requires formatting and script checks, warning-denied host and
x86-64 Clippy, `make check`, every established QEMU smoke target plus
`make input-wait-smoke`, and `make normal-exclusion`. VirtualBox is not required
unless implementation changes the boot path or ordinary image; this feature-only
kernel proof runs in QEMU and must be absent from the ordinary image.

Acceptance must record exact observed counts and independent correctness,
unsafe-boundary, scheduler, privacy, lifecycle, and normal-exclusion reviews.
Passing the proof does not establish authenticated input, a secure shell,
production focus policy, secure erasure, Canticle, exec-hag, or a stable ABI.

## Acceptance evidence

ADR 0022 was accepted on 2026-09-19 against implementation checkpoint
`fcb3578`. The observed acceptance matrix was:

- 17 `anomalous-abi` unit tests plus 5 compile-fail documentation tests;
- 51 `anomalous-session` unit tests plus 56 compile-fail documentation tests;
- kernel host suites with 286 default, 291 circle, 286 control, 297 rite, 292
  scheduler, 292 supervision, 303 shell-vessel, 286 memory, 286 interrupt, and
  303 input-wait tests;
- 38 `xtask` tests;
- formatting, script-syntax checks, warning-denied host and x86-64 Clippy, and
  an explicit all-features bare-metal x86-64 build and strict final ELF link;
- every established QEMU target: `smoke`, `fault-smoke`, `circle-smoke`,
  `control-smoke`, `rite-smoke`, `scheduler-smoke`, `supervision-smoke`,
  `shell-vessel-smoke`, `memory-smoke` at both 96 MiB and 320 MiB,
  `interrupt-smoke`, `input-wait-smoke`, and `shell-smoke`; and
- `normal-exclusion` against the rebuilt ordinary image.

The host/native differential proof covered all 625 four-step traces and every
shared action/origin boundary. It established `u64::MAX - 6` as the last
successful complete wake-delivery starting frontier and `u64::MAX - 5` as the
first failure-atomic live-wait publication rejection frontier. A dedicated
regression rejects a later same-binding wait against stale pending Rite
preparation.

The copy model injected failure at all 280 delivery-body offsets. After every
failure, the reply header remained invalid and exact snapshots proved the
endpoint and all private queue bytes, wait and counters, complete Rite channel,
task table, scheduler, and retained selection reservation unchanged. The
strict QEMU input-wait transcript contained exactly 19 records, including 11
feature-lifecycle records, and disclosed no private or correlating field.

Independent correctness/atomicity, scheduler/lifecycle,
unsafe-boundary/privacy, verifier, feature-precedence, and normal-exclusion
reviews were clear after the reviewed fixes. VirtualBox was not run and was not
required: this removable proof does not change Threshold or the ordinary-image
boot path, and its native lifecycle is exercised under QEMU. ADR 0010 remains
Proposed.

This evidence accepts only the bounded x86-64 mechanism and its test-only
fixture. It does not establish authenticated input, a production session host,
focus policy, a secure shell, Canticle, secure erasure, SMP behavior, IA-32
execution, or a stable ABI.

## Reversal plan

Remove the `input_wait_test` feature, its fixture and verifier, the test-only
endpoint, the semantic-session operation/right and version-1 bodies, and the
ordinary-wake primitive. The accepted physical-input, rite, scheduler,
supervision, shell-vessel, and host-session proofs remain independently usable.
If evidence shows that session input belongs entirely in user-space IPC, retain
the copy-before-consume and exact readiness invariants while replacing this
provisional rite operation before any ABI stabilization.
