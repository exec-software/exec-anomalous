# Initial threat model

## Status and scope

This is a living pre-alpha threat model, not a security claim. It covers the
planned boot, kernel, vessel, control-plane, package, device, compatibility, and
AI boundaries. The current pre-alpha system has no general process isolation,
verified boot, or persistent secrets and must not host sensitive workloads.

## Assets

- integrity of Threshold, the kernel, system policy, and active generation;
- confidentiality and integrity of user data, credentials, model context, and
  enterprise policy;
- availability and recoverability of system and device control;
- authenticity, provenance, and disclosure policy of Reliquary artifacts;
- narrowness and attribution of sigil-granted authority; and
- accuracy of previews, audit events, traces, and postmortem artifacts.

## Principals and trust boundaries

Principals include local people, local and remote agents, enterprise operators,
vessels, device familiars, package publishers, compatibility guests, and remote
services. Identity proves who initiated work; it does not itself grant power.

Important boundaries are firmware to Threshold, Threshold to kernel, kernel to
circle, vessel to vessel, familiar to device, client to semantic control plane,
Covenant to archive, loader to Reliquary, host to compatibility veil, and
exec-hag to untrusted model output or context.

Firmware, hardware, foreign binaries, package archives, network peers,
compatibility guests, embedded artifact context, AI output, and prompts are not
trusted merely because they are well formed or signed.

## Required invariants

- Every privileged operation requires an explicit, suitably narrow sigil.
- Delegation cannot increase authority, and revocation has defined semantics.
- Human and machine clients traverse the same authorization and audit path.
- A preview digest binds the effects that are later authorized and committed.
- Rich untrusted formats are parsed outside the kernel with strict bounds.
- Executable code and AI-facing context carry independent trust decisions.
- Device DMA is confined to explicitly mapped memory.
- Package activation is atomic; rollback never silently corrupts mutable data.
- Compatibility veils cannot mint host identity, sigils, or trusted provenance.
- Diagnostic data observes its own disclosure and retention policy.

## Threats and intended controls

### Boot and kernel

Threats include malicious disk contents, forged handoff fields, integer and
address overflow, unsupported CPU state, firmware compromise, rollback, and
memory corruption. Planned controls include a small versioned `boot_omen`,
bounded validation, signed and measured artifacts, anti-rollback policy,
minimal assembly, documented unsafe invariants, emulator tests, and eventually
hardware-rooted attestation where available.

The implemented version-2 handoff now fails before kernel entry when E820
enumeration cannot prove completion, and the kernel independently copies and
checks the bounded table. Its fixed ledger grants only whole frames from exact
enabled volatile type-1 records after unavailable overlap and low, kernel, and
framebuffer reservations win. This narrows accidental overlap, truncation, and
stale-data risk; it does not make a compromised firmware map truthful.
Threshold requires VBE 2.0 or newer, chooses the version-appropriate legacy or
linear pitch and RGB fields, requires supported graphics and linear-framebuffer
mode attributes, and validates geometry, extent, channel bounds and pairwise
separation, and occupied-memory separation before graphics activation or
framebuffer publication. The kernel independently requires every RGB mask to
fit the declared bit depth without overlap, parses the record fail-closed, and
uses a framebuffer for human-visible failure diagnostics only after it is
structurally safe; COM1 and VGA text remain attributable fallbacks.

### Sigils, IPC, and vessels

Threats include confused deputies, authority laundering, use-after-revocation,
message forgery, denial of service, covert sharing, and supervisor loops.
Intended production controls include unforgeable references, attenuation,
explicit transfer, generation-aware revocation, bounded messages and queues,
quotas, principal and trace propagation, service-health policy, and inspectable
authority graphs.

The fixed proof accepted by ADR 0014 implements a narrower first boundary. It
uses generational tables for 16 circles, 16 tasks, four endpoints, 16 sigil
records, four task-local bindings per task, and four revocation lineages. Only
one task may be running, and every authority lookup revalidates that kernel task
record. A `SIG1`-tagged user reference names a local slot and binding generation
in the current task; it is not a bearer secret or global object identity. The
kernel then validates holder task, circle, fixture principal, sigil and endpoint
generations, known rights, and live lineage. Copying bits from another task does
not copy its binding. Transfer is an explicit failure-atomic move, attenuation
cannot broaden rights, and serialized lineage revocation denies later lookups.

Copied IPC exposes composite authorized operations rather than a crate-visible
unguarded send or receive path. Accepted messages occupy a two-slot fixed FIFO,
receive nonwrapping sequence values, and carry kernel-stamped task, circle,
fixture-principal, and origin provenance from fresh task-table state. Full and
empty queues apply typed backpressure without mutation. Receive preflights the
fixed destination and commits the copy before clearing and consuming the queue
head, so a failed copy retains the message. Kernel-owned slots retain neither a
user pointer nor an alias into sender storage and are volatile-cleared on
consumption or withdrawal.

These mechanisms constrain the local confused-deputy, stale-reference,
post-revocation, attribution-forgery, and partial-copy cases exercised by the
proof. They do not authenticate a human or machine, provide a vessel, prevent a
running task from denying service, or establish a stable IPC or control ABI.

### Covenant and Reliquary

Threats include dependency confusion, compromised publishers or archives,
signature parser bugs, malicious lifecycle work, hidden prompts, provenance
loss, stale vulnerable generations, and storage exhaustion. Controls include
content addressing, threshold signatures and transparency evidence where
appropriate, constrained parsers, independent code/context trust, declared
rites rather than ambient-root scripts, complete plan disclosure, SBOMs,
pinning, quotas, atomic activation, and retained recovery generations.

### AI and semantic control

Threats include prompt injection, context poisoning or exfiltration, fabricated
state, excessive agency, model substitution, unsafe tool composition, and a UI
that conceals machine effects. Controls include treating model text as untrusted
input, schema validation, least-authority sigils, policy gates independent of
model confidence, plan digests, provenance, resource budgets, cancellation,
human confirmation for policy-selected effects, and one audit model for every
client.

### Devices and accelerators

Threats include malicious firmware, DMA outside assigned buffers, side channels,
resource starvation, stale shared memory, and unsafe remote device control.
Controls include IOMMU domains, bounded pinning and zero-copy leases, scheduler
accounting, memory clearing, familiar isolation, schema/version negotiation,
local policy enforcement, and authenticated encrypted transport when remote.

### Compatibility veils

Threats include hostile foreign binaries, syscall/API parser bugs, guest escape,
license violations, provenance confusion, and host integration that weakens
isolation. Controls include explicit compatibility tiers, separate circles or
virtual machines, narrow semantic bridges, immutable base images, origin labels,
fuzzing, and technical plus legal gates for each platform.

The native kernel and rite transport target is x86-64. A future user-space
compatibility veil for IA-32 machine code and the 32-bit Win32 ABI must parse
32-bit pointers, handles, PE/NT structures, and foreign calling
conventions inside its own user-space isolation boundary. It may translate only
validated, copied values into fixed-width native rites; raw foreign pointers or
handles cannot become kernel authority. Supporting 32-bit applications does not
create an IA-32 kernel target.

### Human interface and observability

Threats include deceptive depth or color cues, inaccessible consent,
clickjacking, sensitive logs, forged audit records, and telemetry denial of
service. Controls include redundant state cues, focus and
assistive-technology semantics, trusted confirmation surfaces, tamper-evident
event sequencing, disclosure labels, redaction, retention policy, backpressure,
and separate emergency diagnostics.

ADR 0025 accepts only a host-side fixed semantic-scene proof. Its boundary
addresses projection drift, ghost actions, stale or foreign prepared revisions,
and public reference confusion by deriving the human render plan,
accessibility tree, and machine projection from one immutable privately branded
revision. Exact node, geometry, action, paint, and state budgets are checked
before publication. Move-only instance- and revision-bound handles are required
for local action resolution and focus traversal; raw references, source tags,
labels, coordinates, scene focus, and session-route focus do not authenticate a
principal, select an authority domain, prove presence, or execute an effect.

Protected status bytes remain outside both projections and are replaced with
one fixed placeholder. Different protected values and lengths are tested not to
change geometry, counts, ordering, errors, observations, or action/focus paths.
Fixed one-line UTF-8 rejection rules reduce control, bidirectional, and
invisible-format spoofing, but do not solve homoglyphs or localization. The
content-free observation type is caller-constructible safe shape, not trusted
provenance or an audit receipt.

This evidence does not establish renderer correctness, authentication, policy,
effect binding, secure attention, a deployed accessibility service, side-channel
resistance, secure erasure, or a native desktop boundary. Those remain required
above the accepted host fixture.

### Accepted fixed supervised semantic-renderer composition experiment

[ADR 0026](adr/0026-compose-a-fixed-supervised-semantic-renderer.md) accepts one
removable x86-64 composition proof, not a live desktop service. Its additional
threats include moving canonical scene authority into a renderer, exposing
protected source text through a generated artifact, accepting malformed or
reordered render commands, letting a faulted renderer expose a partial frame,
reusing stale task, channel, stage, or supervisor authority after replacement,
publishing before terminal completion, and allowing feature-only code or data
to enter the ordinary image.

The host build step retains the ADR 0025 scene and both canonical projections.
It emits only a bounded, feature-private capsule containing the already
projected drawing subset and fixed protected-status redaction. The separately
linked ring-3 renderer receives neither the scene owner, machine projection,
action/focus handles, input authority, nor framebuffer mapping; its task-bound
grant is limited to `display_update | task_exit`. Capsule fields, scene and node
numbers, revision values, pixels, and addresses remain correlation data rather
than authority.

Each renderer incarnation writes sequentially into one supervisor-only complete
frame stage rather than the live framebuffer. The first generation's exact
contained fault withdraws and clears its half-complete stage before one fresh
task, channel, sequence, and stage generation is created. Only the replacement's
exact terminal completion can transfer a complete stage to the trusted kernel
publication path. Fallible ownership, geometry, completeness, and presenter
selection checks precede the first live framebuffer write; the final bounded
copy is then non-rejecting in the proof's software model. This is not a hardware
page flip, scanout atomicity, tear-free presentation, DMA isolation, or recovery
from a machine check, power failure, or bug in the final copy.

The fixed renderer and independent host oracle establish exact capsule and
pixel agreement for this one fixture. They do not establish integrity of a
general renderer, a live-scene protocol, arbitrary content, fonts, a compositor,
or a production presenter. The kernel, host build step, oracle, supervision
fixture, final copy, firmware, emulator, and hardware remain trusted within the
stated proof. A malicious or compromised future renderer still requires
isolation plus an authenticated, revision-bound presentation contract; pixel
agreement alone grants no semantic, focus, action, consent, or machine-control
authority.

Protected source value and exact length do not enter the capsule, renderer
image, kernel stage, structured transcript, or final frame. Mutable target and
stage storage is cleared before successful reuse, but this is logical lifetime
evidence rather than secure erasure or side-channel resistance. The one explicit
post-success capture contains only the known public-redaction fixture and grants
no ambient screenshot or retention authority. Dependency, symbol, marker, and
image inspection keep the capsule, renderer payload, gates, and helpers out of
the normal image; ADR 0026 does not add a normal-image renderer runtime.

### Accepted host-only transactional semantic-surface publication experiment

[ADR 0027](adr/0027-prove-bounded-transactional-semantic-surface-publication.md)
accepts a separate `no_std`, allocation-free, unsafe-free host state machine.
It addresses the live transaction threats that ADR 0026 deliberately leaves
open: semantic revision N+1 becoming public before its presentation boundary,
loss or silent replacement of the only prepared candidate, stale renderer work
surviving replacement, replay through copied scalar identities, and divergence
between the accessible and machine projections during publication. It does not
link the service into the kernel, `anomalous-abi`, a vessel, a renderer image,
or the ordinary image and defines no native transport or stable wire format.

One private process-local service brand binds distinct move-only surface and
presentation controls, the exact renderer endpoint, render lease,
`prepared_presentation`, and presentation receipt. Public scene, surface,
renderer, revision, attempt, node, action, geometry, and snapshot values do not
confer authority. Equal public identifiers in separately branded services do
not cross that boundary.
Every transition rechecks the current phase and exact nonwrapping revision,
renderer generation, and delivery attempt; move-only progression prevents a
successful lease, prepared presentation, or receipt from being accepted twice.

The service exposes exactly one complete committed semantic revision and keeps
at most one complete prepared successor private. Queuing, leasing, renderer
readiness, and trusted presentation attestation retain producer backpressure and
do not expose the successor through the public human, accessibility, or machine
views. Only consumption of the exact presentation receipt publishes the complete
prepared scene and all three views together. Existing revision-bound local
action and focus handles therefore remain valid while a successor is pending and
become stale only after successful publication.

The one-slot policy is strict rather than latest-wins. A full-slot, malformed,
stale, foreign, wrong-control, or exhausted submission does not overwrite,
coalesce, reorder, partially install, or compare against the pending candidate.
Its rejection envelope returns the complete move-only candidate to its caller.
Likewise, rejected completion, attestation, publication, and retirement paths
return the lease, prepared presentation, receipt, or service/control owner
needed for an explicit retry or teardown. Error and status metadata format only
as static content-free codes; the returned envelope is still an owner and may
contain private candidate text. It must not be treated as a safe diagnostic
payload, logged, persisted, or transported merely because its error field is
static.

Surface control may replace the renderer once while the first attempt is leased
or ready and before presentation attestation. Replacement preserves the same
immutable redacted snapshot under a fresh endpoint and attempt while making the
old endpoint, lease, and prepared presentation stale. A second replacement is
denied. Once the trusted presentation control attests the prepared
presentation, the service enters a distinct presented phase; renderer
replacement is then forbidden so an already presented frame cannot be
abandoned while the old semantic revision remains public.

The presentation control is an explicit trust boundary, not evidence produced
by the renderer or verified pixels. Renderer readiness proves only ownership
progression, and host attestation does not inspect a framebuffer, authenticate
the presenter, compare pixels, or prove scanout. A dishonest renderer may render
the wrong content, and a dishonest or compromised holder of presentation control
may attest work that never occurred. ADR 0027 proves host-side authority,
freshness, backpressure, restart, and semantic commit ordering only. ADR 0029
now binds one retained transaction to copied transport, task and surface
correlation, renderer completion, framebuffer evidence, and private host
publication in a removable conformance proof. It does not turn the trusted
host assertion into a deployed native attestation service.

Losing or dropping a lease, prepared presentation, or presentation receipt
performs no cancellation, replacement, acknowledgement, or publication. In
particular, a lost receipt leaves the sole successor in the presented phase and
keeps the slot occupied until the exact receipt is published or the complete
service is explicitly retired. This fail-closed ownership rule prevents
implicit replay but does not provide crash recovery or liveness. One pending
producer can hold the only slot indefinitely; the proof provides no fairness,
deadline, admission, quota, cancellation, coalescing, or denial-of-service
containment.

Protected source bytes and exact length do not enter the renderer snapshot,
public projections, static errors, endpoint, lease metadata, prepared
presentation, or receipt. Otherwise identical protected values produce the
same observable snapshot fields and fixed public redaction. This is content
non-disclosure, not activity noninterference: submission, rejection, occupancy,
revision progress,
attempts, restart, retirement, result ordering, resource pressure, and host-added
timing or cadence can reveal that protected work occurred. The service performs
no padding, batching, traffic shaping, telemetry, screenshot, persistence, or
secure erasure.

ADR 0027's evidence is host testing, compile-fail ownership/privacy coverage,
bounded-state checks, dependency inspection, and normal-image exclusion. It
adds no kernel operation, Rite body, user task, framebuffer path, QEMU feature,
or VirtualBox path. Passing the existing native regressions shows that the
host-only crate did not disturb those images; it is not runtime evidence for a
native semantic-surface service.

### Accepted bounded native semantic-surface transport experiment

[ADR 0028](adr/0028-prove-bounded-native-semantic-surface-transport.md)
accepts one removable x86-64 proof of a copied, versioned semantic-surface
body crossing one kernel-owned slot. Separate fixed ring-3 source and consumer
tasks have disjoint submit and read grants; the exact live task incarnations
and private endpoint state authorize the operation, while body identities are
correlation only. The kernel validates only the fixed wire structure and
trusted binding, and the consumer independently validates every defined field
of the delivered profile.

The source-to-kernel and kernel-to-consumer copies are transactional. No
malformed, denied, stale, full, empty, or fallible-copy path may install,
consume, overwrite, coalesce, or expose a partial body. The source task may be
retired before consumer delivery because the endpoint owns the complete copied
body; delivery publishes the destination header last and consumes the slot
once. Teardown withdraws and clears the endpoint, scratch, channels, task
storage, mappings, address spaces, scheduler state, and allocator ownership
before reuse. This is logical lifetime evidence, not secure erasure.

The transcript permits only fixed lifecycle records, aggregate counts, and
invariant booleans. It rejects unknown, reordered, duplicate, malformed,
error, fatal, content-bearing, and correlating records; protected source bytes
and exact lengths enter neither target body, image, result shape, nor evidence.
Activity, ordering, timing, resource pressure, and denial of service remain
visible nonclaims. The feature and its generated payloads, markers, and private
probes remain absent from the ordinary image.

Delivery proves no rendering, framebuffer write, presentation, scanout,
semantic publication, compositor policy, or authenticated service. Delivery
alone must never advance the host transaction; ADR 0029 accepts one separate
bounded composition rather than broadening this transport's authority.

### Accepted bounded native semantic render-and-present composition experiment

[ADR 0029](adr/0029-prove-bounded-native-semantic-render-and-present-composition.md)
accepts one removable successful x86-64 composition. The host retains the
actual ADR 0027 service, surface and presentation controls, and render lease
while its exact ordinal-61 candidate is packaged into ADR 0028's 528-byte
body. Public surface, scene, revision, renderer, attempt, endpoint, task, hash,
transcript, or pixel values remain correlation or evidence only; none can
recreate the private host authority or authorize publication.

The fixed source and renderer have disjoint least-authority grants. Neither
maps the framebuffer or private full-frame stage, and the renderer receives no
scene, action, focus, presentation, or publication policy. A successful render
acknowledgement requires the exact copied delivery owner, task incarnation,
Rite channel, supervisor relationship, stage generation, 768 ordered complete
rows, and one exact successful-exit observation. Partial stages, early exit,
fault outcome, stale or foreign identities, wrong phase, replay, and
caller-selected numeric fields cannot satisfy that conjunction. The dedicated
QEMU run exercises success; these negative acknowledgement properties are
structural and component-tested rather than evidence from a separate negative
x86-64 run.

All fallible feature state is withdrawn before the exclusive presenter copies
the completed stage. The initial framebuffer is a declared mutation oracle,
not a claimed visual rendering of host revision 1. Kernel readback verifies the
final visible channel bytes against that exact stage and preserves baseline
padding; an independent host oracle then verifies all 786,432 RGB pixels.
Only a private move-only verifier result permits the retained controls to
complete, attest, and publish exactly once from revision and publication
sequence 1/1 to 2/2.

At checkpoint `2f453e7`, the strict run produced exactly seven feature records,
777 Rite calls, 775 body copies, 2,392,160 copied body bytes, and 768 rows, then
restored all 65,095 initial free frames. Transcript mutation, altered-frame,
wrong-body, wrong-owner, and incomplete-evidence tests produced no host
publication. Protected source variants produced identical allowed native
artifacts, pixels, transcript shapes, and outcomes; evidence records expose
only fixed lifecycle and bounded aggregate fields. This establishes bounded
content non-disclosure and logical clearing, not secure erasure, activity or
timing noninterference, or side-channel resistance.

The final framebuffer mutation and the host semantic commit are not
crash-atomic. Host failure in between may expose revision-2 pixels while human,
accessibility, and machine readers still observe revision 1, and the proof has
no rollback. It also provides no native retry, scanout or GPU completion,
cross-process presentation authority, stable ABI, compositor, window service,
or normal-image desktop. A deployed service must close or explicitly recover
that crash gap without accepting public evidence as authority.

### Bootstrap command environment

The current bootstrap shell executes in ring 0 and therefore provides no
containment against an implementation defect. It is limited to five local,
read-only display commands, a fixed 128-byte printable-ASCII line buffer, and a
provisional US ASCII adapter over the private interrupt-fed physical-key
channel. It has no filesystem, network, process execution, scripts, variables,
dynamic evaluation, persistence, secrets, or authority operations. Raw scan
bytes, physical keys, command text, and `echo` arguments are excluded from
structured serial telemetry; only controller/source state, exact aggregate
loss, static command classes, bounded sequence values, and outcomes are emitted.

These constraints reduce exposure but do not make the loop a secure shell. The
eventual Canticle parser and session environment must execute in an
unprivileged vessel inside a circle, while the control plane independently
validates policy and sigils. The bootstrap loop must not accumulate privileged
convenience commands while that boundary is unavailable.

### Interrupt, time, and physical input

The accepted BSP-only `legacy_pc` proof owns the emulator's PIC, PIT, and 8042
behind a replaceable backend. It arms only timer and healthy keyboard IRQs after
returning gates, source state, bounded queues, acknowledgement routes, and fatal
fallbacks are installed. IRQ work is allocation-free and bounded. Impossible
state disables maskable interrupts and halts; its static fatal JSON begins on a
fresh serial record boundary so preemption cannot corrupt attribution. Empty
keyboard IRQ storms degrade the input source instead of spinning in interrupt
context, and successful setup or rearm clears the old streak before IRQ1 is
unmasked.

Physical input records have nonwrapping source identity, sequence, and
same-timebase monotonic observation time. A fixed loss latch makes overflow
exact, contains IRQ1, and forces derived keyboard and unfinished shell state to
reset. Rearm requires bounded 8042/device reinitialization before a fresh source
generation restarts at sequence zero. Fixed queues bound memory consumption but
do not prevent denial of service, timing leakage, or deliberate input loss.
Legacy emulated hardware provides neither cryptographic device identity, secure
attention, nor proof of a person.

Steady-state transport, response, and device-error bytes `00`, `ee`, `fa`,
`fc`, `fd`, `fe`, and `ff` invalidate partial decoder framing and force explicit
loss and rearm instead of being published as physical transitions. Byte `aa`
remains intentionally ambiguous with a legitimate left-Shift release; resolving
that ambiguity requires a future device-state protocol rather than inventing
certainty in the decoder.

Scan bytes, physical-key identities, derived text, and shell content remain out
of COM1. The serial port is output-only. Physical transitions are device
observations, not semantic invocations or authorization, and no AI path can
publish fabricated keyboard records. Future human, accessibility, remote, and
AI clients require distinct attributable origins above authenticated IPC and
policy; schema validity or model confidence cannot create authority.

The feature verifier treats every nonempty serial line as protocol: it rejects
unstructured content, malformed records, unexpected events, unknown fields,
failure levels, and private markers. This closes the diagnostic-channel gap in
which an otherwise valid lifecycle could conceal unattributed or newly shaped
content between accepted records.

### Diagnostic exception path

The kernel now replaces the loader GDT, loads a TSS, and installs gates for all
32 architectural CPU exception vectors before following rich `boot_omen`
references. Double fault and NMI receive separate fixed interrupt stacks. The
fatal dispatcher reports static exception identity and bounded processor state;
page faults additionally report CR2 and decoded access bits. It never resumes a
faulting context.

The fault smoke image accepts success only after an invalid-opcode frame matches
the exact vector, instruction address, and code selector armed immediately
before the test instruction. This establishes attribution, not recovery or
containment. Normal images do not contain the test exit behavior.

Instruction and fault addresses can disclose layout. They are acceptable on the
current local bring-up serial channel, which has no secrets or remote sink, but
must become disclosure-governed oracle data before sensitive workloads, address
randomization, persistence, or remote diagnostics exist.

### Fixed circle experiment

The `circle_test` image owns a separate CR3 whose kernel branch is
supervisor-only and whose fixed user branch exposes only distinct RX code and
RW+NX scratch and stack pages. Runtime table walks verify the live effective
permissions and absent guards, with pure permission helpers covered by host
tests. Ring-3 probes are contained only after the
complete lifecycle phase, instruction address, CR2, selectors, stack bounds,
error code, and scratch residue match the expected supervisor-read or NX-fetch
denial. The faulting user frame is discarded.

Every generation restores the original CR3, volatile-clears and verifies its
private mappings and backing pages, clears the TSS privilege stack, and then
constructs a replacement. QEMU and VirtualBox both pass the fixed
three-generation sequence. A normal-image scan rejects the diagnostic gate,
payloads, event namespace, and marker residues.

This is test evidence, not process isolation. It has static pages, fixed
addresses, no allocator, no task identity, no peer domain, no timer or
preemption, no production entry ABI, and no denial-of-service containment. Its
supervisor mappings remain present in the test CR3, and the experiment makes no
claim about speculative leakage, DMA, SMEP, SMAP, or multiprocessor teardown.
Those limits describe the earlier `circle_test` image itself; ADR 0014 adds a
separate bounded control proof without turning either image into a general
process boundary.

### Fixed attributable control experiment

The `control_test` image creates allocator-backed one-task circles with one
private root and three distinct user leaves: RX code, an RW+NX fixed mailbox,
and an RW+NX stack. Exact table walks validate effective permissions and absent
guards before entry. The deterministic BSP sequencer permits only one running
task, keeps maskable interrupts disabled and NMI masked, and never waits or
schedules another context.

Its feature-only DPL-3 gate accepts no user pointer. Each call must match the
kernel-selected current task, lifecycle phase, exact post-`int` instruction
address, selectors, sanitized `RFLAGS`, operation, code bounds, and stack
bounds. The fixed mailbox is reached only through the allocator-authorized
supervisor physical alias. Ring-3 entry clears unspecified registers and FS/GS,
sets `CR0.TS` so the task cannot inherit x87/MMX/SIMD state, and enters with
`IF=0` and `IOPL=0`. User register or mailbox claims cannot replace the kernel
task, circle, principal, origin, endpoint, or sigil lookup.

Nonterminal operations return through `iretq`; only an exact completion
operation abandons the user frame through a single-use saved kernel
continuation while restoring the pre-entry `CR0.TS` state and saved Rust stack.
The kernel then restores its original CR3, clears the active task context, and
wipes the TSS privilege stack before lifecycle withdrawal. That withdrawal
invalidates sigil authority and endpoint state before task storage, removes the
three mappings and address-space root before destroying the task and circle,
and clears code, mailbox, user stack, queue slots, fixed table records, and
saved continuation cells. This ordering limits use-after-revocation,
active-root reclamation, stale queue content, and cross-generation residue; it
does not protect against a malicious ring-0 implementation.

The human and machine producer fixtures execute byte-identical code through the
same gate, composite sigil authorization, FIFO, return, and teardown path. The
kernel stamps distinct synthetic principal and origin values, and the receiver
observes both messages in order after a copied producer-local reference is
denied in its namespace. These labels are deterministic test metadata, not
authentication of a person, model, agent, device, or remote peer.

Acceptance on 2026-09-09 included `make check` with 193 kernel tests and every
QEMU regression. The strict control verifier accepted exactly 28 records in
QEMU with 65,114 free frames restored and the same 28 records in the exact
VirtualBox VM `Anomalous Dev [Threshold]` with 130,667 free feature-image frames
restored. It rejects omitted, reordered, duplicate, malformed, unknown,
unstructured, private, error, and fatal content. `make normal-exclusion`
rejected the feature gate, payload, fixtures, private marker, event namespace,
and test-only paths in the ordinary image; that image was restored and observed
running with 130,683 free frames.

The kernel sequencer, fixed fixture selection, firmware, emulator, and all ring-0
code remain trusted. Threshold's supervisor identity alias remains writable and
executable, so a kernel compromise bypasses the proof. Execution is serialized
on the BSP with NMI masked. The `control_test` image has no scheduler or
preemption; the separate fixed experiment below does not add a general user-copy
or IPC ABI, vessel or supervisor, authenticated identity, DMA isolation, IOMMU
policy, denial-of-service containment, or microarchitectural isolation.

### Accepted fixed x86-64 rite-gate experiment

ADR 0019 accepts one feature-only x86-64 proof while leaving the normal image
unchanged. A fixed ring-3 shell-task fixture models a future supervised vessel,
but this experiment creates no supervision relationship. Future human-facing
shells, AI clients, and exec-hag must all arrive at the same kernel-selected task
incarnation, trusted grant, admission order, and reply semantics; a client label
cannot change authority.

The threats at this boundary include forged caller rights, stale task
incarnations, unknown operation values, size overflow, time-of-check/time-of-use
changes, uninitialized ABI padding, payload reads before admission, diagnostic
exfiltration, and returning to a task after it has exited. The transport accepts
no arbitrary user pointer. It copies one fixed 40-byte request prefix from a
one-page RW+NX mailbox, performs architecture-neutral structural, rights,
incarnation, and kernel-policy length admission, and only then copies an admitted
body once into zero-initialized kernel storage. Its 4,056-byte body cap is
narrower than the kernel's independent 4 KiB rite policy because the call prefix
occupies the rest of the test mailbox. Request, reply, and exit-body layouts have
compile-time checked offsets without implicit padding, and all reserved reply
bytes are written as zero.

The nine-call QEMU sequence rejects an unknown operation, a missing console
right, a display-right overclaim, a 4,057-byte transport overflow, invalid
UTF-8, and a nonzero exit reservation. It then proves a 16-byte
`console_write` commit through `screen_console`, empty `input_read` through the
no-commit `would_block` path, and terminal `task_exit`. All six failures return
payload-free replies. The terminal success copies no reply and consumes the
saved kernel continuation instead of returning through `iretq` to the exited
task.

The strict verifier accepted exactly 20 structured records and rejects omitted,
duplicated, reordered, malformed, unknown, unstructured, error, fatal, selected
field-mutated, and private-sentinel evidence. Serial reports only fixed operation
classes, statuses, sequences, and aggregate lifecycle facts; it does not echo
the private console body, trace identities, registers, addresses, or arbitrary
mailbox bytes. Runtime checks the complete gate frame and every reply, while
host tests cover the ABI layout, grant split, admission ordering, transcript
mutations, and normal-image exclusion.

Successful teardown restores kernel CR3 and `CR0.TS`, clears the active gate
context, transition stack, continuation cells, mailbox, code, user stack, and
private page tables, destroys the task and circle, retires the channel, and
restores all 65,116 initial free frames with zero active/data/table accounting.
The full established QEMU regression matrix, `make check`, `make rite-smoke`,
and normal-image exclusion passed before acceptance.

Standing alone, ADR 0019 reduces the tested risks only inside one serialized
BSP fixture: IF is clear, NMI is masked, and no concurrent writer exists. It
does not prove mailbox TOCTOU resistance under preemption or SMP, a general
user-copy primitive, nonempty input delivery, blocking, scheduling, or
composition with ADR 0018 supervision. ADR 0020 separately tests that fixed
composition, but neither result establishes a user-space shell, authenticated
clients, DMA/IOMMU isolation, denial-of-service containment, or production
security.

### Accepted fixed supervised shell-vessel composition experiment

ADR 0020 composes the scheduler, one-restart supervision relationship, and
task-bound rite boundary in one removable x86-64 image. The threats added by
composition include stale channel authority after destructive replacement,
fixture continuity data colliding with legal rite payload space, a
time-of-check/time-of-use window across real preemption, publication or wake
without terminal task completion, reply copy or `iretq` after exit, supervisor
frame substitution, private projection disclosure, and incomplete reclamation.

The fixed controls bind each channel to one exact running task incarnation and
retire it before identity or storage withdrawal. A replacement receives a new
channel whose sequence restarts at one only after the old task, circle, mappings,
user pages, page tables, and privilege stack are cleared and invalidated.
Transport owns the complete mailbox page; preemption/fault continuity markers
live in a separate fixed stack region. IF is clear throughout every rite gate,
and the proof masks IRQ1 before user execution, so the admitted prefix/body has
no concurrent fixture writer inside the tested BSP model.

Terminal exit uses the admitted task identity for an infallible bounded commit
that marks the scheduled vessel exited, publishes `successful_exit`, and wakes
the exact supervisor. Exact observation consumption and validated selection of
the supervisor frame follow under the same gate exclusion; any mismatch takes a
static fail-stop path. The success reply is never copied and the dead frame is
never restored; no rollback is attempted.

The 38-record QEMU proof includes one real PIT preemption, one exact contained
`#UD`, one restart, five rites, two private console commits with equal fixed
human/machine semantics, two empty-input results, one terminal exit, and two
single-use observations. It restores every initial frame and reports zero active
task, circle, mapping, authority, supervision, scheduler, context, and channel
state. The strict verifier rejects lifecycle, terminal, release, and privacy
mutations. Serial forbids source, output, trace values, mailbox content,
registers, roots, addresses, canaries, and private markers; normal-image
inspection excludes the complete feature.

This narrows only the fixed successful composition. The kernel, firmware,
emulator, fixtures, supervisor identity aliases, and all ring-0 code remain
trusted. One BSP runs with NMI masked; there is no hostile SMP writer, IOMMU/DMA
confinement, microarchitectural isolation, authenticated principal, fairness,
deadline, cancellation, general blocking/input queue, production recovery
policy, or rollback from partial teardown. The fixed projection is not Canticle
or an exec-hag protocol.

The native boundary remains x86-64. IA-32 and 32-bit Win32 execution must remain
inside a versioned user-space compatibility veil that validates foreign pointer
widths, handles, PE/NT structures, object conventions, and calling conventions
before translating them into fixed-width native rites.

### Accepted bounded user-space display presentation experiment

ADR 0024 accepts one feature-only x86-64 presentation proof while leaving the
normal image unchanged. Its fixed ring-3 task holds only
`display_update | task_exit` and maps one RX code page, one RW+NX mailbox, and
one RW+NX stack. The kernel retains exclusive ownership of the framebuffer;
the task receives no aperture mapping, physical address, pitch, or channel
masks.

Threats at this boundary include forged display rights, stale task identity,
malformed or overflowing geometry, payload-length disagreement, live user
mutation after validation, partial framebuffer writes before a late failure,
writes beyond an accepted rectangle, exposure of existing screen content,
private pixel or transport data in telemetry, returning to an exited display
task, and test-only code entering the ordinary image.

The one-page Rite transport admits an exact body and copies it once into fixed
kernel scratch before decoding. The exact 40-byte version-1 header, tightly
packed RGB8 extent, row length, rectangle endpoints, output geometry, and all
integer operations are validated before mutation. The move-only presenter then
converts and writes exactly the accepted rectangle; success is published only
after the full commit. Rejected requests preserve every framebuffer byte, and
copied scratch is logically cleared before reuse or teardown.

The strict proof starts with a deterministic canary across the complete
aperture. Denied-authority, malformed-flags, and out-of-bounds probes each
leave that full aperture unchanged. An explicit walk of the task root requires
the complete identity-mapped framebuffer range to remain supervisor-only. The
vessel then commits 768 full-width rows and exits terminally. Teardown withdraws
and clears its channel, mappings, root, user and page-table storage, scratch,
and transition state while restoring exact accounting.

The accepted transcript has exactly 16 bounded records and contains no pixels,
mailbox bytes, trace identities, addresses, caller labels, or sampled content.
The host captures the declared public 1024x768 frame only after teardown, and
an independent oracle checks all 786,432 RGB pixels. Normal-image inspection
excludes the display gate, fixture, marker, helpers, event namespace, and
test-only payload.

The proof is serialized on one BSP with IF clear and NMI masked. It does not
address hostile concurrent writers, SMP ordering, DMA, IOMMU confinement,
denial-of-service containment, scanout tearing, GPU command submission,
capture authority, secure erasure, or production recovery. The task root
borrows the kernel's supervisor-only low identity branch: architectural CPL3
access is denied, but KPTI and Meltdown-class speculative isolation are not
claimed. Pixels do not confer focus, action, consent, identity, accessibility,
or machine control authority; those require one canonical semantic desktop
model above this boundary.

### Accepted host-only private shell-session ingress experiment

[ADR 0021](adr/0021-prove-bounded-private-shell-session-ingress.md) accepts a
separate `no_std`, allocation-free, unsafe-free host-tested state machine above
the kernel. It models one session and exact consumer, one host-stamped semantic
route, a four-entry private FIFO, bounded typed editing actions, and at most one
pending wait. It is not linked into the kernel or `anomalous-abi` and introduces
no QEMU feature, native input endpoint, gate, wire layout, or normal-image code.

The accepted text path copies a bounded UTF-8 fragment into fixed storage,
rejects the frozen control and format scalar set, separates insertion from
explicit submission, and limits paste to insertion. A pasted line feed remains
data. This blocks paste-triggered actions only inside the proof; it does not
provide trusted paste, focus, secure attention, authentication, parsing,
rendering, evaluation, or system-wide injection resistance.

Private owners are move-only and expose content-derived values only through
explicit local disclosure accessors. Rejected publication returns the owner, and
accepted entries move rather than clone. Errors and the privacy-limited
observation omit content, exact length, origin, identities, and digests, but a
content-free shape authorizes no emission, transport, or retention. Cadence,
volume, externally added timestamps, resource pressure, and denial of service
remain. Logical clearing and `Drop` use ordinary stores and are not secure
erasure of caller copies, compiler temporaries, caches, swap, devices, or
microarchitectural state.

Waiting uses an exact prepare/commit protocol. A nonempty queue dominates a
changed revision, publication enqueues before making an exact wake pending,
receive remains gated until single-use acknowledgement, and close converts a
live waiter into a terminal tombstone. Checked nonwrapping identities, revision
headroom, and same-lineage replacement constrain stale, replayed, duplicated,
wrong-consumer, lost-wake, and cross-generation transitions inside the model.

The origin enum is attribution only. A trusted router outside the crate must
establish principal and route policy before stamping it; sources do not receive
the session owner or choose labels in the proof. The embedding host must
exclusively reserve each session-slot lineage and, while the owner is live,
explicitly close, deliver any terminal notice, and obtain the exact
acknowledgement before destruction. `Drop` cannot perform that protocol or wake
a waiter; abrupt destruction can discard accepted input or orphan a waiter.

The result establishes no authenticated input or focus, secret entry, history,
parser, evaluator, Canticle runtime, kernel nonempty reply, ordinary task wake,
crash-safe session host, side-channel or denial-of-service containment, or
stable session, task, wait, rite, shell, or compatibility ABI.

### Fixed bounded scheduler experiment

ADR 0015's `scheduler_test` image admits exactly two allocator-backed tasks in
separate one-task circles. Each task binds one private root, one exact saved
context, and one distinct 16 KiB supervisor privilege stack for the complete
context lifetime. Every dispatch revalidates that the task identity, CR3,
context frame bounds, user selectors and flags, user RIP/RSP, and live TSS RSP0
all refer to the same task while maskable interrupts are disabled. The active
TSS stack is never cleared, and `CR0.TS` remains set during user execution.

Exactly two armed PIT IRQ0 deliveries may preempt the tasks. After both switches,
only an exact vector-6/error-zero predicate tied to the selected faulting task,
root, frame, instruction, selectors, stack, and private state may fault that
task and resume its peer. Snapshot fields are cross-checked against the live
saved frame. Any other scheduler-active exception emits only a static failure
reason and halts, preventing attacker-selected RIP, RSP, CR2, or register state
from reaching diagnostics. The accepted exception record exposes only its fixed
non-secret fixture RIP/RSP among its address fields and reports a null fault
address.

On the successful path, the peer runs after the fault and returns through a
single-use kernel continuation. The kernel restores CR3, TSS RSP0, TS, and its
saved stack and leaves both PICs in the declared all-masked terminal state
rather than returning to ordinary input delivery. It withdraws contexts, roots,
mappings, allocator leases, tasks, and circles; clears user, page-table, and
privilege-stack storage; scrubs inactive address-bearing metadata; and proves
exact accounting restoration. A setup or teardown inconsistency instead halts;
any ownership not conclusively withdrawn stays allocated and quarantined. This
is a fail-closed non-reuse policy, not leak-free rollback or recovery.

The strict 25-record proof passed QEMU with all 65,112 initial free frames
restored and the exact 512 MiB VirtualBox VM with all 130,665 restored. The
complete prior QEMU regression matrix and normal-image exclusion passed before
the ordinary VM image was restored and left running. These observations cover
the fixed successful lifecycle only.

The scheduler table and switching mechanism are not a production scheduler,
fairness or deadline guarantee, general exception policy, stable task/context
ABI, SMP implementation, FPU/SIMD switch, TLS or per-CPU-base policy, vessel
supervisor, authentication service, or denial-of-service boundary. The terminal
continuation abandons Rust gate frames and is valid only while they contain no
destructors, RAII guards, or lasting borrows. Firmware, emulator, BSP kernel,
fixed fixtures, writable/executable supervisor identity alias, and masked NMI
remain in the trusted or explicitly uncovered base.

### Bootstrap memory ownership

The bounded manager represents every frame below four GiB with unavailable,
free, data-owned, or table-owned state and pairs each live extent with an opaque
allocator- and generation-checked lease. Foreign and bound leases cannot be
released. A virtual owner consumes the noncopyable data lease until unmapping
removes the leaf and binding. Both grant and release clear and verify the
physical extent through the identity map under an explicit unsafe ownership
contract; clearing failure quarantines ownership. The feature proof validates
two complete private table branches, supervisor RW+NX effective permissions,
A-to-B-to-A state separation, explicit rejection of virtual overlap, duplicate
binding, and active-root destruction, root withdrawal before reclamation, and
zeroed same-frame reuse.

The accepted runs cover two QEMU RAM sizes and the dedicated 512 MiB VirtualBox
VM. Their strict transcript gate requires ordered handoff and boot prerequisites,
the exact memory lifecycle, full free-count restoration, and no error or fatal
record. Direct BIOS E820 fault injection and repeated one-time global allocator
publication are not host-executed. Those transitions are reviewed in code;
QEMU and VirtualBox exercise the production initializer, while kernel tests and
emulator gates exercise its fail-closed handoff boundary.

This is ownership accounting, not a ring-0 security boundary. It is BSP-only,
fixed-capacity, and not a general allocator. The bootstrap identity mapping
remains writable and executable, creating a second supervisor alias to every
managed frame below four GiB. There is no IOMMU, DMA exclusion, multiprocessor
locking, TLB shootdown, PCID policy, high-memory support, heap, general process
isolation, or production user-address-space API. Firmware truthfulness remains
trusted after structural checks. Physical addresses appear only in the local
feature test channel and require oracle disclosure policy before sensitive
workloads.

## Current bring-up gaps

The image accepts unsigned code, trusts BIOS-provided platform data after
structural and ownership checks, identity-maps the first four GiB writable and
executable, runs the normal Rust path at ring 0, and has no heap or general
allocator API, production scheduler, general process isolation, IOMMU,
persistent audit log, recovery image, or update mechanism. Its external-
interrupt implementation is a single-BSP legacy-emulator proof, not APIC/SMP
production support. QEMU's
debug-exit port is test-only and is not an integrity signal. NMI remains masked
during the mode transition and after kernel entry; the installed NMI gate and
stack have not yet passed deterministic injection and nested-fault tests.
Threshold still has no exception path before the Rust entry point, and the
kernel's fixed exception stacks lack guard pages. These are explicit bring-up
limitations, not deferred documentation. ADRs 0014, 0015, 0018, 0019, and
0020 provide only fixed kernel feature proofs; ADR 0024 adds a fixed
display-presentation feature proof, and ADR 0026 adds one removable fixed
semantic-renderer composition. Their tables, schedulers, supervision
relationship, rite gates, shell-vessel and renderer fixtures, stages, and
presenters are not a general ABI, production scheduler, vessel runtime,
authentication service, semantic shell or desktop, compositor, or DMA
boundary. The kernel-private PS/2 decoder,
provisional layout adapter, and ring-0 bootstrap shell are development aids
rather than a complete input stack or user-isolation boundary.

ADR 0022 closes the nonempty-copy and exact-wake gap only inside one removable
x86-64 semantic-session feature proof; the ordinary physical `input_read` rite
remains separate. The proof is not a general scheduler path, normal-image
transport, public publisher, or authentication service. ADR 0023 accepts a
host-only router proof that consumes an external authentication assertion, but
the repository still has no deployed trusted origin router, credential
authenticator, focus or secure-attention service, crash-safe session host,
secret-input path, history policy, parser, evaluator, Canticle runtime, or
stable ABI.

ADR 0024 keeps raw framebuffer authority out of ring 3, ADR 0025 proves one
host-only canonical semantic scene, ADR 0026 composes one fixed build-time
capsule with a removable renderer fixture, ADR 0027 proves one host-only
transactional publication state machine, ADR 0028 proves one copied native
delivery between disjoint ring-3 tasks, and ADR 0029 composes one exact live
successor through native rendering, framebuffer verification, and host
publication. The ordinary image still has no deployed semantic object or
window service, user-space compositor, focus or input policy, accessibility
service, display manager, cross-process presentation authority, crash-atomic
pixel/semantic commit, capture policy, KPTI boundary, or stable graphics ABI.
Host-side privacy and machine-readable projection evidence does not prove that
a deployed native desktop preserves those properties or operates without
pixel-derived inference.

Security decisions that settle a boundary require an ADR and tests tied back to
the invariants in this document.
