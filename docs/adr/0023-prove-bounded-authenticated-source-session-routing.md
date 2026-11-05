# ADR 0023: Prove bounded authenticated-source session routing

- Status: Accepted
- Date: 2026-09-19

This record accepts one separate host-only proof that consumes a trusted host
assertion that a source has already been authenticated, binds that source and
its principal to an exact private session route, and stamps semantic origin
without letting the source choose it. One human source follows one explicit
focus transfer between two routes; one machine source remains bound to one
exact route and never inherits human focus. The proof does not authenticate a
credential, identify a real person or machine, decide focus policy, implement
secure attention, or change the kernel or native wire.

## Context

ADR 0021 accepts an allocation-free private shell-session state machine. Its
`prepare_submission` operation is deliberately a trusted-router boundary:
origin is attribution, not authentication, and any embedding host that owns the
session is responsible for principal and route policy.

ADR 0022 accepts a removable x86-64 mechanism proof for a versioned nonempty
semantic-session reply, an owned pending read, and exact wake. Its publisher is
a kernel-authored fixture rather than a deployed router. Version 1 carries only
the action and coarse origin label; it carries no authenticated principal or
source identity.

The next useful proof belongs above both mechanisms. Merely accepting a public
origin enum would let a source call itself human, machine, paste,
accessibility, or remote. Treating a principal number, source number, request
sequence, or copied route identity as authority would turn guessable metadata
into a bearer credential. Preparing an action before revocation and publishing
it afterward would cross an authentication boundary. Reassigning focus without
closing the old session could carry queued typeahead into a new principal or
route.

Implementing passwords, device attestation, remote cryptography, physical
input, a focus manager, and native publication together would conceal those
smaller state-machine obligations. The first proof should instead make the
trusted assertion boundary explicit and test the bounded transitions that
follow admission.

## Decision

### Add a separate host-only proof

Add an `anomalous-session-router` crate beside `anomalous-session`.
It is `no_std`, allocation-free, and unsafe-free, and depends only on
`anomalous-session`. It owns exactly two `shell_session` routes and two fixed
authenticated-source slots: one human-focus source and one machine-explicit
source. It is not linked into the kernel, `anomalous-abi`, a vessel, or the
normal boot image.

The router never exposes either owned session by reference, dereference,
generic callback, `into_inner`, or similar escape hatch. It exposes only the
consumer operations needed to preserve the accepted ADR 0021 wait,
acknowledgement, receive, close, and replacement contract. Every source-side
publication passes through the router.

This is a routing proof, not a new occult abstraction. It uses the conventional
term `session_router`. It is not a familiar because it mediates no device
protocol, not a vessel because it has no service lifecycle, and not a sigil
because its local Rust owners are neither a platform wire capability nor a
delegation format.

### Consume, but do not perform, authentication

Constructing a router returns the router and one move-only `router_control`.
Each successful construction also allocates an unexposed, nonzero,
nonwrapping, process-local instance brand. Every control, source, focus, and
routed-preparation stamp binds that private brand as well as the public router
identity. Two routers with equal public identities therefore cannot exchange
authority. Brand exhaustion rejects construction; the brand is not serialized,
stable across process restart, cryptographic, or a public identity.

Construction rejects an equal session-slot lineage or equal consumer-value
lineage between A and B inside one router, including aliases from different
generations. The trusted host must reserve both complete lineages across every
concurrently live router because this crate has no global route registry.

Only router control may admit or revoke a source or change human focus. The
trusted host calls a deliberately named admission operation only after an
external authentication decision has succeeded. The operation accepts these
asserted facts:

- one nonzero generational principal identity;
- one exact vacant source slot selected by the trusted host.

The human-focus slot fixes `human_text` origin and focus-following routing. The
machine-explicit admission operation has no route parameter: it fixes
`machine_text` origin and the current exact route B. No source receives an API
for choosing its origin or routing mode.

Successful admission allocates one nonzero generational source identity and
returns a move-only `host_verified_source` authority. Its name records the
trusted assertion consumed by this proof, not credential verification. It binds
the exact router incarnation, principal, source generation, origin, and next
nonzero request sequence. The source cannot alter any of those parts or use the
authority with another router. Controller-authorized revocation selects the
fixed source slot by kind and does not require possession or cooperation of the
source token. It invalidates the exact source generation before any later
preparation or publication can succeed, but does not close a route, revoke
focus, or retract content already accepted into a session queue.

Principal, source, route, and request identities remain non-secret correlation
and freshness data. Equal numeric values grant no authority. The local
move-only owners are unforgeable only inside this safe-Rust proof; they are not
serialized credentials, cryptographic authenticators, kernel sigils, or proof
that the asserting host made a correct real-world decision.

### Bind two routes and one human focus domain

The router owns exact routes A and B. Human admission creates no focus by
itself. A live human binding is required before controller activation, which
has no target parameter and creates the initial move-only A
`focus_authority`. The only transfer consumes that exact A authority and
returns the exact B authority; repeated or reverse transfer is rejected. A
live human binding is also required at transfer. The active focus binding
captures the focus generation, session identity, consumer identity, and route
epoch. Human preparation then combines that binding with the independently
validated principal, source generation, and fixed human origin.

The machine-explicit source is independently bound to exact route B. Its
binding captures the exact principal, source generation, machine origin,
session identity, consumer identity, and route epoch, but not the human focus
generation. It never follows or inherits human focus. Before transfer, human
input targets A while machine input targets B; afterward both may reach B only
through their distinct exact source bindings.

Human and machine sources use the same preparation, publication, queue,
backpressure, wait, and receive mechanisms after route resolution. Their fixed
origins and per-source request sequences remain distinct local attribution.
Neither identity, origin, nor route mode grants authority.

An external focus or policy service may tell `router_control` to perform the
one transfer, but this proof neither observes a desktop focus state nor decides
that policy. “Human focus” here means only the exact A-to-B selection enforced
by this state machine; it says nothing about machine route authorization.

### Prepare and publish as two exact phases

Source publication is split into two move-only phases.

Preparation validates the complete live source authority and its routing mode,
then captures the source's current exact-next request sequence and the private
semantic action. A human preparation captures the exact current focus
generation and route. A machine preparation captures exact route B and is
independent of human focus. A successful preparation returns a
`prepared_routed_submission` containing:

- the exact router, principal, source, and applicable focus generations;
- the exact session, consumer, and route identities;
- the exact source request sequence; and
- the move-only semantic action.

Preparation does not consume the source request sequence, and more than one
preparation may capture the same current value. Publication revalidates that
the captured value is still next, then invokes the owned session's accepted
prepare-and-publish path. Only successful session publication advances the
source cursor, so another preparation with the old value becomes stale. Queue
pressure returns the owner with the same retryable sequence. The maximum value
may publish once and then marks that source generation exhausted; no value
wraps or resets before a fresh source generation. Request sequence remains
correlation only and confers no authority.

Publication revalidates every captured value before it enters the ADR 0021
queue transition. Revocation, session replacement, a stale source, wrong
router, or an exhausted cursor rejects without changing session state. Focus
transfer additionally makes a prepared human-A action stale, while a prepared
machine action for unchanged route B remains valid. Queue pressure and every
other rejection return private action ownership. Queue pressure may retry the
same routed preparation and sequence. A stale-focus rejection requires fresh
preparation against the new focus but preserves the same unconsumed source
sequence and action owner. A successful publication uses the target session's
existing FIFO, submission sequence, revision, and exact-wake rules; the source
request sequence does not replace any of them.

The first proof is serialized. It establishes no lock, cross-thread sharing,
SMP ordering, asynchronous credential refresh, or distributed revocation
semantics.

### Retire A before completing the human focus transfer

The only focus transition is `focused(A) -> focused(B)` through one move-only
focus authority. Its commit closes A with `route_replaced`, logically clears
and counts A's queued typeahead, advances the focus generation, and publishes
the exact B focus authority. Every existing human-A preparation is then stale.

Before closing A, the router validates that exact B is live, open, and empty,
and preflights the next nonwrapping focus generation. Those fallible target and
generation checks precede the A close. The underlying close can still reject,
for example on revision exhaustion, but is itself failure-atomic; the router
then returns the unchanged A authority and changes neither route nor focus.

Route B is not closed, replaced, or relabeled by this operation. A prepared
machine action bound to unchanged B therefore remains valid and retryable
throughout the human focus transfer, and new human preparation resolves to B
under the next focus generation. If A had a waiter, the returned close receipt
preserves its exact terminal tombstone; that wake must be acknowledged before A
may be replaced or destroyed, but it does not revoke the already established B
focus.

Source revocation is separate from focus transfer. It invalidates that source's
later preparation and publication but does not retract already accepted queue
content. Explicit retirement closes the affected exact route when clearing
accepted content is required. A revoked source slot cannot bind a new principal
to the same live route incarnation: human reuse requires focus retirement, and
machine reuse requires close-and-replace of fixed route B. The machine source
cannot hop through A to bypass this rule. Repeated transfer or retirement is
idempotent or returns one static typed error; it never creates overlapping
human focus.

After human focus reaches B, that route is shared with any live machine
binding. Focus revocation then returns `shared_route_active` with the unchanged
focus authority and does not alter either source or B. The host must explicitly
revoke the machine slot first, then revoke focus to close and clear B. Thus
ordinary human focus transfer never changes machine authority, and shared-route
retirement requires an explicit coordinated controller decision.

Dropping the router or a private preparation uses the underlying logical-clear
path as a privacy fail-safe. It is not protocol close and does not establish
secure erasure. The host must complete explicit retirement while a live wait
or route exists.

### Keep principal attribution local

Within this proof, move-only routed preparations and local publication receipts
may expose their exact principal, source, request, and target-route attribution
through named local disclosure methods. The session queue itself retains the
accepted origin and action contract. The proof does not claim that principal or
source attribution survives native or independent session delivery.

No principal, source, focus generation, or source request sequence is added to
ADR 0022's version-1 delivery. The native body continues to carry only action
and origin. Connecting local
principal attribution to a future user-space or kernel transport requires a
separate versioned decision and cannot be inferred from matching integers.

Errors use static content-free codes, and this proof adds no ambient
content-observation or telemetry API. Named lifecycle getters expose public
router/session/consumer/route identities, while named local disclosures on
private owners expose only the attribution or action discriminator explicitly
requested by the caller. Errors contain no private text, action or origin tag,
principal or source identity, route or request sequence, exact length or erase
count, timestamp, excerpt, or digest. A safe format authorizes no telemetry
sink or retention.

## Required invariants

- Only the move-only router control admits or revokes sources. Control creates
  the initial A focus authority; transfer and revocation consume the exact live
  focus authority.
- A source cannot self-select its principal, origin, session, route, or focus.
- Numeric identities and sequences never confer authority.
- Every preparation and publication binds the exact live principal/source,
  routing mode, session, consumer, and route.
- Successful focus transfer closes and clears A before installing B. Later
  rejection of an old human-A preparation is failure-atomic, while an unchanged
  machine-B preparation remains valid.
- A source request sequence is exact-next, advances only after successful
  publication, never wraps, and is never accepted twice.
- Publication preserves ADR 0021 ownership return, FIFO, revision, wait, and
  close guarantees.
- Human focus transfer closes and clears A before human input can resolve to B.
- An A terminal waiter is acknowledged exactly once before A replacement or
  destruction.
- Machine-explicit B authority never derives from human focus and is unchanged
  by the A-to-B transfer; shared-B retirement requires explicit machine
  revocation before focus revocation.
- Human and machine sources follow the same mechanism while retaining distinct
  attribution.
- Local principal attribution never appears in the current native delivery or
  privacy-safe observation surface.

## Non-goals

This decision does not define or implement:

- password, passkey, biometric, device, workload, model, remote-peer, or
  cryptographic authentication;
- principal enrollment, account recovery, credential storage, attestation,
  trust roots, or authentication policy;
- focus discovery or arbitration, secure attention, trusted paste, secret
  entry, accessibility-service trust, or UI indicators;
- a physical-input familiar, keyboard/layout path, clipboard service, remote
  transport, or public publisher;
- a kernel change, new rite, sigil, ABI field, wire version, native principal
  delivery, QEMU feature, or normal-image integration;
- more than two fixed routes, more than one human-focus source, more than one
  machine-explicit source, fan-out, broadcast, fairness, threads, SMP, or
  distributed revocation;
- a crash-safe runtime, vessel lifecycle, persistent history, encrypted state,
  audit retention, side-channel resistance, or secure erasure;
- Canticle parsing, machine AST input, evaluation, authorization policy, or a
  secure shell; or
- a production session host, stable authentication API, or compatibility ABI.

## Acceptance requirements

Host tests must cover:

- nonzero, stale, wrong-router, replaced, exhausted, and nonwrapping router,
  principal, source, focus, route, and request identities, including duplicate,
  replayed, retryable, maximum, and exhausted request sequences;
- both fixed source slots and both exact session routes, including
  failure-atomic admission, revocation, and reuse boundaries;
- fixed human and machine origins, equal downstream mechanics, and proof that a
  source cannot choose or change its origin or routing mode;
- inactive, revoked, wrong-principal, wrong-source, wrong-route, and stale
  authorities with private ownership returned and state unchanged;
- the exact A-to-B focus transfer with empty, queued, prepared, and blocked A;
- a prepared human-A action becoming stale while a previously prepared
  machine-B action remains valid and retryable;
- exact terminal acknowledgement before A replacement, complete A queue
  clearing, and rejection of every old human-A preparation after transfer;
- queue pressure plus session sequence and revision exhaustion without partial
  router or session mutation;
- exact local attribution without adding it to the native version-1 body;
- exhaustive enumeration of the declared bounded critical routing subset, plus
  directed admission, held-preparation, revocation, wait, acknowledgement,
  cancellation, retirement, and replacement tests; and
- static content-free errors against private sentinels and absence of an
  ambient observation surface.

The declared exhaustive subset is all `7^4 = 2,401` four-step post-admission
traces over immediate human prepare/publish, immediate machine prepare/publish,
one focus-transfer attempt, A receive, B receive, machine revocation, and
eligible B retire/replace/re-admit. This does not claim exhaustive coverage of
admission, held preparations, human or focus revocation, wait lifecycle, or A
replacement; the directed tests above cover those transitions.

Compile-fail documentation tests must independently show that router control,
focus authority and receipts, source authority, routed preparations, private
rejections and deliveries, and the router owner cannot be cloned, formatted,
or hashed accidentally. Scalar identities remain explicitly documented as
non-authority. The crate must pass warning-denied Clippy with unsafe code
forbidden.

Acceptance requires `make check`, the established QEMU regression matrix, and
normal-image exclusion. No router-specific QEMU or VirtualBox result is
required because the crate changes no boot, kernel, ABI, or image path. Passing
these tests establishes only the bounded local routing
contract, not correct real-world authentication or focus policy.

## Acceptance evidence

ADR 0023 was accepted on 2026-09-19 against implementation checkpoint
`83caf3f`. The observed acceptance matrix was:

- 17 `anomalous-abi` unit tests plus 5 compile-fail documentation tests;
- 51 `anomalous-session` unit tests plus 56 compile-fail documentation tests;
- 24 `anomalous-session-router` unit tests plus 46 independent compile-fail
  ownership/privacy tests;
- kernel host suites with 286 default, 291 circle, 286 control, 297 rite, 292
  scheduler, 292 supervision, 303 shell-vessel, 286 memory, 286 interrupt, and
  303 input-wait tests;
- 38 `xtask` tests;
- focused warning-denied router rustdoc plus repository formatting,
  script-syntax checks, warning-denied host and bare-metal x86-64 Clippy, and an
  all-feature x86-64 kernel build;
- every established QEMU target: `smoke`, `fault-smoke`, `circle-smoke`,
  `control-smoke`, `rite-smoke`, `scheduler-smoke`, `supervision-smoke`,
  `shell-vessel-smoke`, `memory-smoke` at both 96 MiB and 320 MiB,
  `interrupt-smoke`, `input-wait-smoke`, and `shell-smoke`; and
- `normal-exclusion` against the rebuilt ordinary image.

All 2,401 declared bounded routing traces passed. Directed tests separately
covered both source slots and every semantic action, same-public-ID router
separation, cross-kind and cross-router denial, equal wait/wake and backpressure
mechanics, controller revocation, route replacement, stale focus, source and
focus exhaustion, close-failure atomicity, terminal acknowledgement, and
shared-B revocation ordering. Final independent architecture, correctness, and
security/privacy reviews were clear after the reviewed fixes.

VirtualBox was not run and was not required because this host-only crate changes
neither Threshold nor the kernel, ABI, vessel graph, or ordinary image. This
evidence accepts only the bounded local routing contract; it does not validate
the external authentication decision, define focus policy, carry principal
attribution over ADR 0022's native body, or deploy a production session host.

## Consequences

The project gains a small falsifiable boundary between an external
authentication/focus decision and the existing private session mechanism. A
source cannot mint its own origin or carry prepared input across revocation,
human typeahead cannot cross focus replacement, and machine authority cannot
silently inherit a person's focused route.

The proof also makes the remaining work more explicit. A real authenticator,
focus service, input familiar, user-space runtime, principal-bearing transport,
and native integration still require independent threat models and evidence.
The fixed capacities and local Rust authorities remain removable scaffolding.

## Alternatives considered

### Treat `submission_origin` as authentication

Rejected. It is caller-shaped attribution data and contains no proof of who
selected it.

### Use a principal or source number as a bearer credential

Rejected. Copyable identifiers provide correlation and freshness, not
authority or revocation ownership.

### Add routing directly to `anomalous-session`

Rejected for the first proof. Keeping the router in a dependent crate preserves
the distinction between private session mechanics and authentication/focus
policy and makes bypasses visible at the ownership boundary.

### Put the router in the kernel or extend the version-1 delivery

Rejected. The authentication assertion and focus decision are user-space policy
concerns, while a principal-bearing wire requires independent versioning and
privacy analysis. ADR 0022 should remain a removable mechanism proof.

### Implement credential verification in this slice

Rejected. A fixed password, key, or fixture would demonstrate comparison, not
production authentication. The router instead names and tests the trusted
assertion boundary honestly.

### Generalize beyond two routes and two sources immediately

Rejected. Exact A-to-B human focus and one independent machine-B binding are
enough to prove source binding, differentiated route selection, stale
preparation rejection, queue clearing, and orderly retirement before adding
multiplexing or policy.
