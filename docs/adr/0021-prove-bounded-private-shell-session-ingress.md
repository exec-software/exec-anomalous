# ADR 0021: Prove bounded private shell-session ingress and waiting

- Status: Accepted
- Date: 2026-09-13

This record accepts one host-only, allocation-free proof of private semantic
input, bounded queueing, and lost-wake-safe session waiting. Its capacities,
action vocabulary, origin labels, identities, and observation shapes are
removable pre-alpha evidence. It is not Canticle syntax, a stable session or
input ABI, an authentication service, or a kernel wait interface.

## Context

ADR 0013 keeps scan bytes and physical key transitions private to the kernel
input mechanism and explicitly separates physical input from human or machine
authority. ADR 0020 then composes a scheduled shell-shaped vessel with native
rites and supervision, but its two `input_read` calls find an empty source and
return `would_block`. It does not deliver nonempty input or block a shell task.

A real shell session cannot safely consume a global physical-key stream. That
would turn ordinary input authority into a keylogger across focus, principals,
and sessions. An input familiar and trusted session host must eventually route
semantic text and editing actions to one exact session while preserving their
origin. Machine clients must use their own attributable ingress rather than
fabricating keyboard provenance.

Session content is unusually sensitive. Source text, object names, completion
queries, command timing, exact lengths, errors, history, and even an unkeyed
digest of a common command can disclose intent or credentials. Automatic
history, broad crash capture, ambient environment inheritance, or content-rich
telemetry would reproduce failure modes the native design is intended to avoid.

Waiting also creates a correctness and security boundary. A naïve
"check empty, then sleep" sequence can lose input that arrives between those
steps. A copied or stale wake identity can resume the wrong task or a replacement
session. Queues that overwrite on pressure, pasted newlines that implicitly
submit, and typeahead that crosses a crashed-session boundary can duplicate or
execute work without an attributable decision.

The current kernel mechanisms cannot honestly close these gaps in one edit:

- the accepted rite admission is a synchronous guard that closes if abandoned
  and cannot remain borrowed across a context switch;
- the current `input_read` shape has a zero-byte request and only a
  `would_block` completion;
- the x86-64 transport writes only the fixed reply prefix and has no accepted
  nonempty reply-body copy/consume transaction; and
- the scheduler's exact wake composites are coupled to supervision fault and
  completion transitions rather than a general input-ready wake.

The accepted experiment is therefore a host-tested session state machine.
A later ADR must define the native input-reply and kernel wait transactions
before any of this policy crosses the x86-64 rite boundary.

## Decision

### Scope and placement

Add a `no_std`, host-tested `anomalous-session` crate. It is not linked into the
kernel or `anomalous-abi` and introduces no QEMU feature, syscall, gate, wire
layout, executable format, or foreign compatibility surface.

The crate uses fixed backing storage, checked fixed-width logical identities,
no heap allocation, and no unsafe code. It models exactly one open shell session,
one exact consumer incarnation, one semantic input route epoch, up to four
queued actions, and at most one pending wait. Those proof limits are not policy
or ABI.

`session` remains a conventional engineering term. This decision introduces no
new occult platform noun.

### Session and route identity

A session has a nonzero generational `session_identity`, an exact nonzero
`consumer_identity`, a pinned provisional language version, and a nonzero route
epoch. These values are host-owned attribution and freshness inputs. They are
not authentication, authority, sigils, globally meaningful object identities,
or values that an input source may select.

Every prepared submission and wait captures the exact session and route
incarnation. Closing invalidates old submissions and uncommitted wait
preparations. If one exact wait is already blocked or wake-pending, its identity
survives only as a terminal tombstone until the same consumer acknowledges the
close.

Within one owner lineage, replacement is a checked closed-session transition.
It refuses to reopen while a terminal tombstone remains, preflights nonwrapping
advances of the session generation, consumer generation, and route epoch, then
opens empty storage with fresh submission/wait sequences and revision. No old
identity or queued typeahead can cross that checked boundary.

The crate contains no global identity registry. Initial construction therefore
has an explicit trusted-host precondition: the session slot is an exclusive
lineage anchor, and the host must never assign that slot or any of its generation
namespace to another independent owner, live or retired. Constructing an
independent owner inside an existing lineage defeats freshness and is outside
this proof. Consumer and route values are still checked as part of every exact
incarnation. All of these values remain non-secret and confer no authority.

The proof contains no current directory, environment map, inherited descriptor
set, executable search path, implicit sigil namespace, wall clock, randomness,
network, filesystem, or persistent history. Later session context must add such
state as typed, inspectable objects and explicit authority rather than ambient
strings or process inheritance.

### Typed semantic input

The session accepts semantic editing actions, never scan codes, controller
bytes, device timing, or physical-key records:

- `insert_text` carries one nonempty bounded UTF-8 fragment;
- `erase_previous` carries one nonzero bounded scalar count;
- `submit` is an explicit action separate from text insertion;
- `cancel_submission` is an explicit editing action distinct from wait
  cancellation; and
- `source_discontinuity` is a typed barrier that tells the consumer to discard
  unfinished derived editing state before later input.

A text fragment is at most 256 bytes, is copied into fixed private storage, is
never silently truncated or normalized, and has a zeroed unused tail. The exact
proof policy accepts valid Unicode scalar values plus literal U+0009 tab and
U+000A line feed. It rejects every other Unicode control scalar, plus U+00AD,
U+034F, U+061C, U+180E, U+200B through U+200F, U+202A through U+202E,
U+2060 through U+206F, and U+FEFF. Malformed UTF-8, empty input, and over-limit
input also fail before publication. Text insertion never becomes `submit`; a
pasted line feed remains data.

The proof origin classes are `human_text`, `machine_text`, `paste`,
`accessibility`, and `remote_text`. They are distinct host-stamped attribution
labels, not authenticated principals or authority. The submission-preparation
entry point is a trusted-router API: an untrusted source does not receive the
session owner or select its own label in this model. Passing an origin enum
authenticates nothing; a later router must establish principal and route policy
before stamping it. Origin never changes queue, wait, or privacy behavior. The
`paste` origin is structurally limited to `insert_text`; it cannot construct
`submit`, `erase_previous`, `cancel_submission`, or
`source_discontinuity`. A later machine AST path must remain distinct from
physical input and converge only after parser/decoder validation; this proof
does not select that encoding.

Input action contents are data only. This crate does not parse, interpolate,
split, expand, render, evaluate, invoke a rite, or interpret any text as
executable source.

### Privacy boundary

All action content is session-private by default. The private text owner is
move-only and implements none of `Clone`, `Copy`, `Debug`, `Display`, `Hash`, or
serialization. Every content-derived accessor is a deliberate local disclosure
boundary, including text, exact byte length, action kind, and erase count. An
authorized consumer can borrow and then copy or format revealed values, but the
private owner does not do so implicitly. Rejected publication returns ownership
to the caller so queue pressure never silently discards private work. Accepted
queue entries move rather than clone the content.

Errors are bounded static codes and contain no source excerpt, object name,
identifier, origin-supplied string, pointer, exact content length, or digest.
The privacy-limited observation shape reports an event class, result class,
`content_redacted=true`, and `history_persisted=false`. It carries no session,
consumer, route, submission, wait, origin, timing, or content-derived identity.
The shape does not authorize emission, transport, or retention: event cadence,
volume, and external timestamps can still disclose activity. More detailed
receipts remain explicit local values without formatting or hashing traits.

There is no history, clipboard, oracle, accessibility projection, remote
forwarding, crash attachment, or persistence API in this proof. Those are
separate disclosure operations requiring policy. Accessibility input is an
origin class; that does not authorize disclosure to an accessibility service.

Private buffers and unused tails use one explicit logical-clear path during
queue consumption, session close, and drop. Tests can verify zeroed live owned
storage after explicit clear, consumption, and close; safe Rust cannot inspect
memory after drop. The types have `Drop` implementations that call the same
path, but ordinary stores may still be optimized or copied. This is not a
secure-erasure claim for caller copies, compiler temporaries, allocator
remnants, caches, swap, devices, or microarchitectural state.

Rust `Drop` cannot return a close receipt or deliver a terminal wake. It is a
privacy fail-safe, not a protocol transition. Queue-delivery and waiter-outcome
guarantees apply while the session owner remains live and uses the explicit
state-machine operations. The embedding host must close, deliver any terminal
notice, and allow its exact acknowledgement before destroying the owner.
Abrupt owner loss may discard accepted input or orphan a waiter and is outside
this proof; a later supervised runtime must close that lifecycle gap.

### Queue and publication

Within the live session transition API, the queue is FIFO and never
overwrites or silently drops an accepted entry. Explicit session close is that
API's sole destructive discard: it clears queued content and returns a local,
non-formatting close receipt with the exact number of discarded entries.
Privacy fail-safe owner destruction is separately constrained above. An action
receives a unique nonzero submission sequence only after every route, state,
capacity, and sequence check passes. Full queue, closed session, stale route,
invalid consumer, and sequence exhaustion leave all session state unchanged and
return the move-only prepared submission.

Publication always makes the complete action visible in the queue before it
produces a wake notice. Source code cannot choose a consumer or wake identity.
A source discontinuity occupies its own FIFO position; later text cannot pass
it.

### Lost-wake-safe waiting

Waiting is an explicit two-step protocol:

1. `prepare_receive` observes an empty open queue and returns a move-only
   preparation containing the exact session, consumer, route, proposed wait
   sequence, and current session revision.
2. `commit_wait` revalidates the identities and proposed wait sequence
   before its ready path. If that sequence is still current and input arrived
   after preparation, the nonempty queue dominates the changed revision and
   forces a ready result instead of blocking. A stale sequence is rejected even
   when later input exists. Only an unchanged empty session installs one exact
   blocked wait.

Publishing to a blocked session first enqueues the action, then changes that
wait to `wake_pending` and returns its exact `wait_identity`. Receive is
rejected while a wait is blocked or wake-pending; only acknowledgement of that
exact input-ready wake returns the open session to a receive-eligible state.
Only the exact consumer may acknowledge that wake. A copied identity confers
no authority: at most one equal identity can complete the current wait, and all
remaining copies become stale after that completion. Wrong-session,
wrong-consumer, stale-sequence, replayed, duplicate, and already-consumed
identities fail without mutation.

Wait cancellation may remove only the exact still-blocked wait. It cannot
cancel an already pending wake or consume queued input. Closing linearizes at
one explicit state mutation: it clears and counts all queued entries, converts
an existing blocked or input-ready wake into a terminal wake on the same wait
identity, and prevents later publication. The exact consumer acknowledges that
tombstone once, but terminal acknowledgement never enables receive. A waiter
therefore receives exactly one input-ready or terminal outcome and is never
silently orphaned by close.

Preparation is read-only. Each accepted publish, receive, wait installation,
wake acknowledgement, wait cancellation, and first close advances the session
revision exactly once; a publish that also makes a wake pending remains one
transition. Rejected work and repeated close do not advance it.

Ordinary mutating transitions reserve two further revision values for an
explicit close plus possible terminal acknowledgement. Close itself requires
one remaining value without a waiter and two with one; terminal acknowledgement
may consume the last value. Thus no publicly reachable accepted transition can
strand private input or a waiter merely by entering the end of the revision
space. Once the reserve is reached, other mutation fails unchanged with
`revision_exhausted` and the owner must retire through close. Replacement
resets revision only after advancing all freshness identities. Session revision,
submission sequence, wait sequence, identity generation, and route epoch never
wrap; exhaustion is explicit and failure-atomic.

## Required invariants

- Names, text, origin labels, session identities, consumer identities, route
  epochs, wait identities, lengths, and safe observations confer no authority.
- No machine-origin submission can claim to be a physical keyboard event because
  the contract contains no physical-input variant.
- Every accepted action is complete, bounded, private, attributable, and
  assigned one nonreused sequence.
- Rejected input consumes no sequence and changes no queue, wait, or revision.
- While the owner remains live, FIFO actions are never silently overwritten,
  dropped, duplicated, or reordered; explicit close alone discards cleared
  entries and reports a count.
- Text insertion cannot implicitly submit, including when it contains a pasted
  line feed.
- A wait binds the exact live session, consumer, route, revision, and one
  nonreused sequence.
- Input arriving before wait commit prevents blocking; input arriving after
  blocking is queued before the exact wake becomes pending.
- Wake acknowledgement and cancellation are exact, single-use, and
  failure-atomic; equal copies confer no authority, receive cannot race ahead of
  acknowledgement, and a terminal acknowledgement never reopens receive.
- Every accepted ordinary mutation leaves enough revision headroom for close
  and, when needed, terminal acknowledgement.
- Checked same-lineage replacement requires a closed, fully acknowledged owner
  and advances all freshness identities before reopening empty; exclusive
  session-lineage allocation remains a trusted-host obligation.
- Privacy-limited observations contain no private content, correlator, exact
  length, excerpt, or unkeyed digest and authorize no sink or retention.
- No ambient context, history, authority, evaluator, effect, or kernel surface
  is introduced.

## Non-goals

This decision does not define or implement:

- Canticle grammar, a parser, formatter, AST, machine encoding, resolver,
  type/effect checker, evaluator, renderer, or semantic control plane;
- authenticated people, machines, agents, devices, remote clients, focus, secure
  attention, trusted paste, or accessibility-service identity;
- a global identity allocator, authenticated origin router, crash-safe owner
  lifecycle, or enforcement of the host's exclusive-lineage and
  close-before-destroy obligations;
- secret entry, declassification, clipboard, history, completion, oracle,
  telemetry retention, persistence, or encryption;
- a kernel input endpoint, nonempty rite reply, blocking rite, ordinary scheduler
  wake, user-copy primitive, signal, timeout, deadline, or cancellation ABI;
- arbitrary tasks, multiple consumers, multiple waiters, broadcast, fan-out,
  fairness, SMP, APIC, NMI enablement, DMA/IOMMU isolation, or side-channel
  resistance;
- POSIX process state, shell reparsing, environment or descriptor inheritance,
  Windows process tokens or handle inheritance, registry/search-order behavior,
  or desktop-global input/clipboard/UI-automation trust; or
- a stable session, input, shell, task, wait, rite, executable, package, or
  compatibility ABI.

The native kernel and eventual rite transport remain x86-64. IA-32 machine code
and the 32-bit Win32 ABI remain isolated user-space compatibility-veil concerns
that must validate and copy foreign values before native translation.

## Acceptance requirements

Host tests must cover:

- exact text bounds, UTF-8 validity, exhaustive frozen-scalar policy,
  zero tail, explicit logical clearing, explicit content/length/count
  disclosure access, and non-formatting private ownership;
- every action and origin class, with paste/newline separate from submit and
  all four non-insertion paste pairings rejected;
- FIFO order including a wrapped/straddled ring, full-queue backpressure,
  returned ownership, sequence exhaustion, no partial mutation, source
  discontinuity ordering, close clearing and discarded-count reporting;
- empty poll, wait preparation and commit, input before preparation, input
  between preparation and commit, input after block, stale preparation replay
  against later input, exact wake acknowledgement, cancellation, close wake,
  and input-ready/terminal races;
- wrong session, consumer, route, revision, wait, stale generation, duplicate
  use, replay, checked same-lineage replacement, and exhaustion with unchanged
  state;
- exact revision increments and end-of-space retirement for every mutating
  transition, including cancellation, both acknowledgement outcomes, and close;
- content-free error and privacy-limited observation formatting against private
  sentinels, without treating that format as permission to emit; and
- exhaustive bounded action traces over publish, prepare, commit, wake,
  cancellation, receive, and close transitions.

Compile-fail documentation tests must show that private content and local
receipts cannot be cloned, formatted, or hashed accidentally. The crate must
build and test under `no_std` with unsafe code forbidden and pass
warning-denied Clippy.

Acceptance also required `make check`, every established QEMU smoke proof,
and normal-image exclusion. Because this slice changed neither boot nor
kernel code, VirtualBox is useful but not required for acceptance.

Acceptance records exact observed test counts and keeps ADR 0010 Proposed. A
passing session state machine does not establish a secure shell, authenticated
input, kernel wait ABI, Canticle, exec-hag, or production confidentiality.

## Acceptance evidence

ADR 0021 was accepted at implementation checkpoint `a2c72b2`. The observed
acceptance matrix was:

- 8 `anomalous-abi` tests;
- 51 `anomalous-session` unit tests and 56 independent compile-fail
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

Final independent privacy/security and correctness audits were clear. VirtualBox
was not run and was not required because this slice changed neither boot nor
kernel code. ADR 0010 remains Proposed.

This evidence accepts only the bounded host-side state-machine contract. It does
not establish secure erasure, authenticated origin or authority, nonempty kernel
input delivery, a kernel wait transaction, Canticle, a secure shell, or
production confidentiality.

## Consequences

With this decision, Anomalous gains a small, testable contract for routing
private semantic input and sleeping without a lost-wake window. It also makes
explicit which state belongs above the kernel and which missing kernel
transactions must be designed separately.

The cost is another fixed state machine and a provisional action vocabulary.
Both can be replaced before ABI stabilization. The next kernel-facing decision
must specify a versioned nonempty input-reply shape, copied-body commit order,
task-local readiness generation, general exact wake transition, revocation and
focus-loss behavior, and complete clearing without holding a rite admission
across a context switch.

## Alternatives considered

### Give the shell the physical input queue

That would expose system-wide keystrokes and device metadata across focus and
session boundaries. The input familiar should route semantic actions instead.

### Make `input_read` block inside its existing rite admission

The admission guard owns synchronous mutable channel state and intentionally
closes if abandoned. Holding it across a task switch would create borrow,
lifetime, stale-task, and teardown hazards.

### Return nonempty bytes without a wait model

That proves copying but leaves polling, lost wakes, cancellation, focus changes,
and replacement typeahead unresolved.

### Use source strings for machine control

Machines may submit source for compatibility, but the native machine path should
use a validated typed tree. Neither source nor a future tree can carry authority.

### Enable history and redact later

Redaction cannot reliably recover secrets after source, errors, traces, and
crash artifacts have already been copied. Persistence remains absent until its
encryption, retention, principal, disclosure, and deletion policy is designed.
