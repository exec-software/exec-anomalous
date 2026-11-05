# ADR 0014: Prove fixed attributable sigil-gated IPC

- Status: Accepted
- Date: 2026-09-09

This record accepts one deliberately fixed, feature-only task, circle, sigil,
and IPC experiment. Its table capacities, virtual addresses, gate vector,
message shape, status values, role fixtures, and event names are removable
pre-alpha evidence, not stable kernel or control-plane ABI.

## Context

ADR 0011 proved that a fixed x86-64 protection domain can enter ring 3, receive
exact access denials, be torn down, and restart from cleared storage. ADR 0012
then established allocator-owned frames and bounded address-space ownership.
ADR 0013 added returning interrupts, monotonic observations, and private physical
input while reserving AI activity for a future authenticated semantic path.

The kernel still has no task identity, general circle lifecycle, IPC endpoint,
or sigil enforcement mechanism. The experimental `invocation_header` in
`anomalous-abi` is layout scaffolding: its principal and sigil-shaped fields are
caller-controlled claims, not authenticated identity or bearer authority.
Treating that record, a numeric syscall, a copied token, or a Rust object name as
authority would establish a confused-deputy boundary before the kernel can
identify who is actually running.

A pure host-side model is necessary for exhaustive state testing, but it cannot
prove that ring-3 registers or memory fail to forge task provenance or install
authority. Conversely, adding a scheduler, arbitrary user buffers, a stable
syscall ABI, full semantic invocations, or a vessel supervisor at once would
hide the first authority and copying invariants under too much unproven policy.

## Decision

### Scope

Implement a `control_test` feature and bounded `make control-smoke` proof on the
bootstrap processor. The feature uses a deterministic kernel sequencer with
maskable interrupts disabled and NMI masked. It creates allocator-backed,
one-task circles and runs one task at a time to completion. This is cooperative
one-shot execution, not scheduling or preemption.

The architecture-neutral core uses fixed-capacity tables for circle, task,
endpoint, sigil, and local-binding state. It allocates no heap memory, accepts no
unbounded collection, and never wraps an identity generation or message
sequence. The x86-64 harness binds that model to real CPL-3 entry, separate page
tables, a feature-only DPL-3 gate, fixed mailboxes, and destructive teardown.

Normal images contain the reusable fixed-table mechanisms only when referenced;
they contain no test gate, payload, role fixture, private marker, forced state,
or `kernel.control_test.` event.

### Circles and tasks

A live circle record has a nonzero, nonwrapping slot and generation. In this
experiment it owns exactly one allocator-backed address-space root, one RX code
page, one RW+NX mailbox page, one RW+NX stack page, absent guards, one local
sigil-binding namespace, and at most one task. The address-space owner consumes
the corresponding leases and validates effective permissions and the complete
expected table shape before activation.

A task identity is independently generational and is bound immutably while live
to its circle, synthetic fixture principal, origin class, code bounds, stack
bounds, and lifecycle state. Only the kernel sequencer selects the current task.
User registers and mailbox bytes cannot select or replace its task, circle,
principal, or origin. The permitted lifecycle is `ready -> running -> exited`;
invalid transitions fail without mutation.

Circle destruction requires no live task, endpoint, sigil binding, active root,
or owned mapping. Authority is invalidated before an address space is withdrawn.
CR3 returns to the kernel root before any mapped or table lease can be cleared
and released. Code, mailbox, stack, transition-stack, queue, and table storage
are cleared and verified before reuse.

### Task-local sigils

A sigil reference exposed to a task is an opaque, task-local slot and binding
generation with a format tag. It is not a global object identifier, secret bit
string, or self-authenticating token. The kernel resolves it only in the current
task's private binding table and then validates the referenced kernel-only
record:

- live nonwrapping sigil and lineage generations;
- holder task, circle, and fixture principal;
- exact endpoint identity and generation;
- known rights sufficient for the requested operation; and
- a live, non-revoked lineage.

Copying or guessing another task's reference bits does not copy its binding.
At most, those bits can name authority already installed independently in the
current task's own namespace.

Root issuance is callable only by the kernel bootstrap sequencer in this proof.
Explicit transfer first installs an equivalent target binding and only then
invalidates the source, or makes no change. Attenuation requires the `delegate`
right, preserves target and lineage, and permits only a strict or equal subset
of known rights. Ordinary IPC never installs or transfers authority.

The first revocation rule is deliberately coarse and serialized: revoking a
lineage invalidates its root and every derived binding on the next lookup.
An operation whose authorization completed before revocation may finish;
revocation neither rolls back completed work nor promises selective descendant
revocation. Repeated revocation is a typed result, never generation reuse.

### Copied IPC endpoint

The proof uses one receiver-owned endpoint with two inline message slots. Send
and receive are nonblocking. A full send returns `would_block`; an empty receive
returns `would_block`. Neither operation overwrites, silently drops, waits, or
uses the input event channel's loss-latch semantics.

An accepted send copies one fixed typed payload into kernel-owned storage and
then assigns the next nonwrapping endpoint sequence. The kernel stamps source
task, circle, fixture principal, and origin from the current task record. No
caller-supplied field can override that provenance. The queue retains no user
pointer or shared alias, and sender teardown cannot change an accepted message.

Receive requires the current task's exact receive authority and endpoint
ownership. The kernel validates the fixed destination mapping and completes
copy-out before consuming the queue slot. FIFO order is exact. A failed send or
receive leaves depth, indexes, next sequence, output, and authority state
unchanged. Cleared slots must not retain the private feature marker.

The fixed payload and status words are feature-local test data, not a rite,
invocation, RPC schema, general byte stream, or stable wire representation. This
decision does not consume or stabilize `anomalous_abi::invocation_header`.

### Ring-3 binding

The feature installs a dedicated DPL-3 interrupt gate distinct from ADR 0011's
diagnostic vector and every returning device IRQ. Entry validates the active
task, CPL-3 selectors, sanitized RFLAGS, code and stack bounds, requested
operation, and exact lifecycle phase. The gate accepts no user pointer. It
accesses only the current circle's already validated, fixed mailbox through its
allocator-authorized supervisor physical alias.

The task enters with `IF=0`, `IOPL=0`, `DF=0`, null FS and GS, cleared general
registers except explicit test inputs, and `CR0.TS` set so inherited x87, MMX,
and SIMD state is unavailable. A nonterminal gate call returns with `iretq`.
The exact completion operation abandons the user frame and returns through a
single-use saved kernel continuation. Any unexpected gate, exception, state,
selector, address, or status is fatal.

Current external-interrupt dispatch validates kernel-origin frames, so enabling
interrupts in user mode is outside this proof. The single fixed TSS privilege
stack also cannot retain blocked task continuations; no task may wait or yield.

### Coequal mechanism evidence

The sequencer first prepares a persistent receiver circle, one task, and its
two-slot endpoint. It then executes the same immutable producer payload twice:

1. a fresh producer circle and task receive a kernel-authored `human_fixture`
   role and an exact send binding;
2. after complete authority invalidation, root withdrawal, and verified clearing,
   a replacement producer circle and task receive a `machine_fixture` role and
   an equivalent exact send binding.

Both role fixtures run byte-identical code, present the same invalid local
reference and receive the same denial, send the same typed payload through the
same authorization path, observe return to user mode, and complete. Their
accepted records occupy endpoint sequences zero and one. The first message must
remain immutable after its sender's complete destruction.

The receiver then enters ring 3. A copied producer reference is denied in the
receiver namespace. Its installed receive binding yields sequence zero followed
by sequence one, with equal payload and distinct kernel-stamped task, circle,
principal, and origin attribution, then returns `would_block` on the empty
queue. It observes no inherited producer binding.

`human_fixture` and `machine_fixture` are deterministic test labels selected by
the kernel. They do not authenticate a person, model, agent, or external client.
This proof establishes equivalent mechanism and distinct provenance only; it
does not establish semantic control-plane equivalence or policy.

### Observable lifecycle

The strict feature transcript reports only static roles, counts, generations,
sequences, lifecycle states, and booleans. It never emits payload bytes, the
private marker, sigil-reference bits, user registers, object or physical
addresses, or caller-supplied identity fields.

The ordered lifecycle must establish:

1. normal boot, memory, display, and fault prerequisites;
2. fixed capacities, no scheduler, disabled user interrupts, and exact mapping
   counts;
3. a receiver-owned endpoint and receive-only binding;
4. human-fixture task entry, invalid-reference denial, sequence-zero send,
   return to user mode, exit, and verified teardown;
5. a clean replacement producer with a new circle/task/binding generation;
6. byte-identical machine-fixture execution and sequence-one send;
7. receiver entry, cross-namespace denial, FIFO delivery of sequences zero and
   one, equal payload, distinct kernel attribution, and exact empty result;
8. lineage revocation taking effect before any later lookup;
9. endpoint, task, circle, mappings, page tables, mailboxes, queue slots, and
   transition-stack state withdrawn or cleared in dependency order;
10. exact restoration of initial free, data-owned, and table-owned frame counts;
    and
11. bounded test success through the feature-only QEMU exit path.

## Required invariants

- User state never supplies authoritative task, circle, principal, or origin.
- A sigil reference resolves only through the current task's live binding table.
- Names, payload values, object IDs, and experimental invocation fields confer
  no authority.
- Transfer and attenuation are explicit, bounded, rights-nonincreasing, and
  atomic on failure.
- Revocation has one serialized boundary and invalidates every later lookup in
  the selected lineage.
- Endpoint and binding identities include nonzero generations that never wrap or
  become valid again after exhaustion.
- IPC copies fixed values, retains no user pointer, preserves FIFO, and applies
  backpressure without partial mutation.
- A message sequence is assigned only to an accepted enqueue and never reused.
- User mappings are allocator-owned, effectively permission-checked, never W+X,
  and contain no peer or arbitrary physical mapping.
- Authority dies before task or circle storage; CR3 withdrawal precedes frame
  reclamation; cleared storage is verified before reuse.
- Human and machine fixtures receive equivalent scoped authority and outcomes
  through byte-identical producer code while retaining distinct attribution.
- COM1 remains output-only and carries no authority material or message content.

## Non-goals

This decision does not define or implement:

- a scheduler, preemption, context switching, blocking, wakeup, deadlines,
  fairness, priorities, or multiprocessor synchronization;
- multiple concurrent tasks, multi-task circles, or arbitrary executable
  loading;
- a stable syscall, IPC, task, circle, sigil, message, or wire ABI;
- caller-selected pointers, buffers, lengths, shared memory, zero-copy, streams,
  routing, discovery, or general RPC;
- general process, vessel, supervisor, crash-restart, or resource-accounting
  policy;
- authenticated human, machine, model, or remote identities;
- semantic rites, objects, discovery, preview, commit, cancellation,
  idempotency, compensation, audit persistence, Canticle, or exec-hag;
- sigil persistence, serialization, message-borne transfer, selective
  descendant revocation, or distributed revocation;
- user-mode interrupt delivery, recoverable arbitrary faults, NMI enablement, or
  denial-of-service containment for looping code;
- DMA isolation, IOMMU policy, KPTI, ASLR, SMEP, SMAP, speculative-channel
  resistance, or production security; or
- changes to the normal bootstrap shell or physical input authority.

## Security limitations

The kernel sequencer, fixed role fixtures, emulator, firmware, and all ring-0
code remain trusted. Deterministic local references are nonforgeable only by
kernel-mediated namespace lookup and memory isolation, not entropy or
cryptography. A compromised kernel can manufacture records or authority.

Only one task executes at a time with interrupts disabled. A looping or hostile
task can deny service indefinitely, and the fixed gate is a feature-test escape,
not a production syscall boundary. The current kernel identity map retains
writable and executable supervisor aliases to managed frames. There is no DMA
or microarchitectural isolation.

Endpoint sequence and generation checks prevent silent local reuse; they do not
provide semantic replay protection, invocation idempotency, authenticated audit,
or recovery after an uncertain remote transport. Fixture principal values are
kernel-owned metadata but are not rooted in an authentication service.

## Acceptance evidence

Host tests must cover every fixed-table capacity and generation boundary,
including nil, out-of-range, wrong-kind, wrong-task, wrong-circle, stale-slot,
stale-object, stale-lineage, revoked, and exhausted references. They must cover
all known-right subsets, unknown rights, missing delegation, attempted
broadening, explicit transfer, target-full rollback, repeated revocation, and
teardown ordering.

IPC tests must cover zero capacity, empty, full, FIFO and index wrap, sequence
exhaustion, wrong receiver, wrong target and right, immutable copying, queue-slot
clearing, and unchanged state for every failed operation. Task and circle tests
must cover every valid transition plus duplicate, out-of-order, nonempty, and
stale destruction attempts. Address-space tests must cover three distinct leaves
within one owned branch, exact effective permissions, absent guards, duplicate
virtual addresses, branch mismatch, capacity, W+X rejection, active-root
destruction, withdrawal, and complete clearing.

`make control-smoke` must accept only the exact ordered structured lifecycle and
QEMU debug-exit status 33. Its verifier rejects missing, duplicate, reordered,
unknown, malformed, unstructured, error, fatal, private-marker, handle, address,
register, or payload-shaped serial content. Feature images must time out or stop
safely on any unexpected user return, exception, reset, signal, or exit.

Normal-image symbol and byte inspection must reject the gate, user payloads,
test entry and completion paths, role fixtures, event namespace, private marker,
and test-only force/drain/revoke helpers while retaining reusable core
mechanisms. Acceptance additionally requires `make check`, every existing QEMU
regression, the same exact feature lifecycle in the dedicated VirtualBox VM,
and restoration of the normal interactive image afterward.

## Acceptance record

Accepted on 2026-09-09 after implementation checkpoint `7663420` and final
invariant-hardening checkpoint `ba5f513` were committed and pushed. `make check`
passed with 3 `anomalous-abi` tests, 193 kernel tests, 22 `xtask` tests, and
target Clippy with warnings denied. Every required QEMU target passed: boot,
fault, circle, control, memory, interrupt, and shell smoke, followed by
normal-image exclusion.

The strict QEMU control run accepted exactly 28 structured records and restored
its initial 65,114 free frames. The dedicated VirtualBox VM
`Anomalous Dev [Threshold]` reproduced the exact 28-record lifecycle and
restored its initial 130,667 free frames. Both control runs completed without an
error or fatal record. The ordinary image was then rebuilt and restored to the
same VM; it reported 130,683 free frames, reached `bootstrap_shell.ready`
without an error or fatal record, and remains running.

Independent final review found and closed five authority, integrity, and
evidence gaps
before acceptance. Raw queue mutation is module-private, and callers use
composite operations that join current-task authorization to send or receive so
the structure cannot be bypassed accidentally. Validation covers every occupied
queue record rather than only queue indexes and depth. Sigil allocators exhaust
fail closed instead of exposing a partially allocated generation or lineage.
Receiver and final accounting evidence is derived from the preceding lifecycle
rather than accepted as an independent self-report. An exhausted endpoint must
also retain the canonical maximum next-sequence value, so consistently altered
sequence state fails closed. The final emulator evidence was generated from
that hardened tree.

This acceptance remains limited to the fixed feature-only proof. Synthetic
human and machine fixtures establish distinct kernel attribution and equivalent
mechanism, not authentication. The result defines neither a scheduler nor a
stable task, sigil, IPC, syscall, or control-plane ABI, and it does not establish
a general vessel runtime.

## Consequences

The project gains its first end-to-end connection between allocator-owned ring-3
execution, authoritative task provenance, local capability lookup, and copied
message delivery. It also gains bounded host-testable state machines that later
scheduling, syscalls, supervision, and semantic control can build on without
treating caller claims as identity.

The cost is another feature-only gate and recovery path, a multi-mapping
address-space refactor, more fixed kernel tables, and a larger exact-test
surface. None of those details become stable ABI. The next gate must decide how
general task contexts, preemption, safe user-buffer copying, and recoverable
faults replace this deterministic sequencer.

## Alternatives considered

### Stop at a host-side model

This can validate table transitions but cannot show that ring-3 callers fail to
forge provenance, authority, or kernel queue ownership.

### Treat the experimental invocation header as authenticated

Its bytes are caller controlled. Syntax and layout validation cannot turn its
principal or sigil-shaped fields into kernel authority.

### Use a global capability number or random bearer token

A global number is transferable by copying, while early deterministic entropy
would not establish cryptographic secrecy. Task-local bindings make transfer a
kernel operation and remain meaningful without randomness.

### Reuse the input event channel

That channel deliberately records loss after dropping on pressure. Authority-
bearing IPC instead needs explicit backpressure and no accepted-message loss.

### Allow arbitrary user buffers now

Safe hostile-pointer copying needs permission walks, bounded lengths, fault
fixup, and partial-copy rules. A fixed validated mailbox isolates that separate
boundary from the first authority proof.

### Add a scheduler first

Preemption, saved contexts, timer delivery in user mode, blocking, fairness, and
task-stack ownership would multiply the state space before basic attribution,
authorization, copying, and cleanup are established.
