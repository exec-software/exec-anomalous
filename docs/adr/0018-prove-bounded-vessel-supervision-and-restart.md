# ADR 0018: Prove bounded vessel supervision and restart

- Status: Accepted
- Date: 2026-09-09
- Accepted: 2026-09-13

This record accepts one deliberately fixed, feature-only supervision
experiment. Its roles, event vocabulary, gates, restart budget, payloads, and
execution trace are removable pre-alpha evidence. They are not a stable vessel,
supervisor, scheduler, syscall, package, executable, or control-plane ABI.

## Context

ADR 0015 proved that a real PIT interrupt can preempt one isolated user task,
that a second task can run with a private saved context, and that one exact
user-mode invalid instruction can fault only its task while the peer remains
viable. It deliberately excluded blocking, wakeup, supervision, restart, and
general user-space services.

Anomalous now needs the smallest mechanism that can eventually move persistent
services out of ring 0. In the accepted occult vocabulary, a restartable
user-space service is a `vessel`. A useful vessel boundary must preserve one
supervisor while replacing a failed task and isolation circle; changing a
faulted task back to ready would preserve stale execution state and would not
prove replacement.

Destructive replacement cannot safely run on a task's privilege-transition
stack. The current x86-64 proof also keeps the mutable frame allocator borrowed
by the suspended kernel call. Publishing that borrow through a raw runtime
pointer or freeing the stack beneath the exception handler would introduce
aliasing and active-stack hazards. The experiment therefore needs a deliberate
checkpoint back on the original kernel stack before it erases a failed
generation.

## Decision

### Scope and names

Add a `supervision_test` feature and `make supervision-smoke` proof on the
bootstrap processor. The proof uses exactly two kernel-authored roles in
separate one-task circles:

- `vessel_fixture`, whose first task and circle generation faults and whose
  second, freshly constructed task and circle generation completes; and
- `supervisor_fixture`, whose task, circle, saved context, address space, and
  privilege stack remain live across that replacement.

The feature introduces an architecture-neutral fixed-capacity supervision
table and opaque generational `supervision_id`. The identity names a kernel
relationship; it is not authority, authentication, a package identity, or a
stable external vessel identity. The word `vessel` is used because the fixed
child models a supervised, restartable service. No synonym for the accepted
term is introduced.

Human- and machine-directed workloads use the same mechanism. This proof does
not encode either client class in the scheduler, fault classifier, or restart
policy.

### Blocked task state

Extend the task lifecycle with `blocked`:

```text
ready -> running -> blocked -> running
running -> faulted
running -> exited
```

A blocked task remains live and retains its saved context, address space,
privilege stack, endpoint bindings, and existing authority. It is not
dispatchable, is not current, appears zero times in the runnable queue, cannot
exercise authority, cannot receive newly issued authority, and cannot be
destroyed or withdraw its context. Ordinary scheduler selection must never turn
`blocked` into `running`.

Only checked composites may make a blocked task running in this experiment:

- block the current supervisor and directly dispatch its exact active vessel
  task;
- fault the exact active vessel task, publish one kernel-owned observation,
  consume the supervisor's exact wait, and directly dispatch that supervisor;
- park the current supervisor with no runnable peer so execution can return to
  the kernel checkpoint; and
- complete the replacement vessel task, publish one completion observation,
  consume the supervisor's exact wait, and directly dispatch that supervisor.

Every composite preflights both task records, saved-context ownership, current
identity, queue topology, relationship phase, observation capacity, and
nonwrapping sequences before its first mutation. Failure leaves task,
scheduler, and supervision state unchanged. No standalone user-selected wake
target, token, task identity, or reason is accepted.

The no-peer park transition represents `current = none` and an empty runnable
queue. The kernel is not represented as a phantom scheduled task.

### Supervision relationship and observations

Each live relationship records exactly:

- the exact supervisor task and circle;
- the active vessel task and circle incarnation, if any;
- a bounded lifecycle phase;
- one restart budget, initialized to exactly one;
- one optional kernel-owned observation slot; and
- nonzero, nonwrapping event and restart sequences.

The supervisor circle owns one `supervision` child in circle accounting. The
vessel circle remains a disposable one-task circle and is a validated referent,
not a second owner of the relationship. This lets the failed circle be destroyed
without weakening the existing rule that a task is its circle's only remaining
child at task destruction.

An observation contains only its sequence, the exact recorded incarnation, and
one coarse outcome: the fixed invalid-instruction fault class or successful
exit. It never contains an exception frame, register, arbitrary fault address,
payload, message, user string, or private memory. Only the exact running
supervisor may consume it, and consumption is exactly once. A pending
observation prevents the supervisor from blocking, so a wake cannot be lost.

The first consumed fault observation authorizes one restart transition. That
transition consumes the complete restart budget and detaches the old
incarnation from the relationship. A replacement must be a newly allocated,
ready task in a newly allocated circle. The old task and circle identities must
be stale, the new generations must advance without wrapping, and scheduler
admission must succeed before the replacement is published as active. A
faulted task is never reset to ready.

This proof gives the fixture no outward message capability. Endpoint queues
owned by a retired receiver are cleared through existing withdrawal, while
messages already copied into foreign queues would intentionally survive sender
teardown and are therefore outside this experiment's cleanup claim.

### Exact x86-64 lifecycle

Reuse the accepted 22-qword, 176-byte saved-context layout and common interrupt,
exception, gate, and `iretq` epilogues. Do not introduce a second frame format.
The execution is split into two user sessions around a kernel checkpoint:

1. construct the first vessel incarnation and persistent supervisor, validate
   their private roots, user pages, privilege stacks, contexts, relationship,
   and saved kernel continuation, and dispatch vessel generation 1 with IF
   clear;
2. let generation 1 initialize private state and enable IF through an exact,
   pointer-free feature gate;
3. accept exactly one real PIT IRQ0, advance monotonic time, acknowledge the
   master PIC once, mask both PICs, save and suspend generation 1, and dispatch
   the supervisor's initial frame;
4. let the supervisor initialize a private canary and invoke the exact
   `await_fault` gate, atomically blocking it and resuming the saved vessel;
5. require generation 1 to verify its private state and execute exactly one
   `ud2`;
6. validate the complete exception predicate, atomically fault generation 1,
   publish the coarse fault observation, wake the exact blocked supervisor, and
   pivot the exception epilogue to its saved frame;
7. let the supervisor verify its unchanged state, consume that observation,
   and invoke the exact restart-request gate;
8. block the supervisor with no runnable peer, unpublish the task runtime,
   restore kernel CR3, bootstrap TSS RSP0, original TS, and the saved kernel
   stack, and return a distinct restart checkpoint status;
9. only on that restored kernel stack, withdraw and clear generation 1's
   context, mappings, address space, user frames, privilege stack, task, and
   circle; require its old identities to be stale; then construct generation 2
   with fresh task and circle identities and cleared private storage;
10. bind and admit generation 2, establish a fresh single-use kernel
    continuation, select its root/frame/TSS state, and republish the runtime;
11. dispatch generation 2 with IF clear, require it to observe clean storage,
    and accept its exact completion gate, which exits it, publishes the
    completion observation, wakes the blocked supervisor, and pivots to the
    supervisor's preserved frame;
12. let the supervisor verify continuity, consume the exact completion
    observation, and complete through the second single-use kernel
    continuation; and
13. restore kernel processor state, withdraw and destructively clear generation
    2 and the supervisor, remove and scrub the relationship, restore all
    accounting, emit the deferred transcript, and terminate the proof.

The saved generation-1 PIT frame may retain IF set after the physical sources
are masked. All other user transitions keep IF clear. The live TSS RSP0, CR3,
selected frame, task identity, relationship transition, and stack bounds must
agree before every user return. CR0.TS remains set during both user sessions.
The two saved kernel-continuation cells are single-use and are volatile-cleared
after consumption.

No failed generation storage is cleared until kernel CR3, bootstrap RSP0, and
the original kernel stack have been restored. No supervisor storage is cleared
while its saved continuation remains usable. Runtime pointers and gate/switch
reentry latches are unpublished before either nonlocal return and are
republished only after the next complete selection is valid.

### Ownership and failure policy

All setup and replacement construction retains explicit ownership of every
lease until publication. A construction failure must either unwind in reverse
dependency order to the exact starting accounting or keep the incomplete
ownership quarantined and halt. Sequential erase, unmap, destroy, detach, or
release failures are terminal; they must never report a successful restart or
make ambiguous storage allocatable.

The successful proof must restore initial allocator, mapping, address-space,
task, circle, context, scheduler, endpoint, sigil, lineage, supervision, and
circle-child accounting. Every retired user page, page-table page, privilege
stack, inactive record, pending observation, wait association, and
address-bearing runtime field is cleared before reuse or final release.

### Observable lifecycle

The strict transcript contains exactly 33 structured records: the eight normal
boot records through `kernel.interrupts.ready`, followed by these 25 records:

1. `kernel.supervision_test.ready`;
2. `kernel.supervision_test.vessel.generation.created` for generation 1;
3. `kernel.supervision_test.supervisor.ready`;
4. `kernel.supervision_test.task.dispatched` for vessel generation 1;
5. `kernel.supervision_test.task.preempted` for the one real PIT tick;
6. `kernel.supervision_test.task.dispatched` for the supervisor;
7. `kernel.supervision_test.supervisor.blocked` for `await_fault`;
8. `kernel.supervision_test.interrupts.masked`;
9. `kernel.supervision_test.task.resumed` for vessel generation 1;
10. the standard `kernel.exception` record for the exact contained user `#UD`;
11. `kernel.supervision_test.vessel.generation.faulted`;
12. `kernel.supervision_test.task.resumed` for the supervisor;
13. `kernel.supervision_test.observation.consumed` for the fault;
14. `kernel.supervision_test.restart.requested` at the kernel checkpoint;
15. `kernel.supervision_test.vessel.generation.withdrawn` for generation 1;
16. `kernel.supervision_test.vessel.generation.created` for generation 2;
17. `kernel.supervision_test.task.dispatched` for vessel generation 2;
18. `kernel.supervision_test.vessel.generation.completed`;
19. `kernel.supervision_test.task.resumed` for the supervisor;
20. `kernel.supervision_test.observation.consumed` for completion;
21. `kernel.supervision_test.supervisor.completed`;
22. `kernel.supervision_test.vessel.generation.withdrawn` for generation 2;
23. `kernel.supervision_test.supervisor.withdrawn`;
24. `kernel.supervision_test.released`;
25. `kernel.supervision_test.passed`.

Only static roles, generations, bounded sequences, transition names, coarse
outcomes, aggregate accounting, and fixed booleans may appear. The standard
exception record may additionally report its ELF-derived fixed RIP, fixed user
RSP and selectors, null fault address, and RFLAGS with only the architectural
Resume Flag variance already accepted by ADR 0015. The transcript excludes
physical/root/frame addresses, pointers, general registers, canaries, mailbox
contents, payload bytes, secrets, and arbitrary user-provided text.

The final summary derives and requires exactly six dispatches, one preemption,
three resumes, two blocks, two wakes/observations, one contained fault, one
restart, and one clean replacement completion. Tick values may vary but the
single reported PIT tick must be positive.

## Required invariants

- Blocked is live but never ordinarily dispatchable or runnable.
- A task becomes running from blocked only through an exact, kernel-owned,
  single-use supervision observation transition.
- A pending observation cannot be overwritten, skipped by waiting, or consumed
  by any task other than the exact running supervisor.
- Relationship identity is attribution, not authority; every operation also
  validates the current task, incarnation, phase, and circle generations.
- Fault publication, observation wake, task transitions, queue selection, and
  dispatch-sequence allocation are one failure-atomic composite.
- Restart means destructive retirement followed by a fresh task and circle;
  stale identities and private state cannot cross that boundary.
- No destructive replacement work executes on a task transition stack or
  through a published frame-allocator alias.
- Every selected frame, root, and TSS RSP0 belongs to the same exact task and is
  revalidated before return.
- The persistent supervisor's saved context and private state survive both
  wakes and the kernel restart checkpoint.
- Diagnostic output is deferred until the final kernel-stack return and
  discloses no private task state.

## Non-goals

This decision does not define or implement:

- a production scheduler, general wait queue, arbitrary wake source, timeout,
  cancellation, signal, fairness, priority, deadline, or multiprocessor policy;
- a general vessel manager, stable vessel identity, service manifest, loader,
  package lifecycle, dependency resolver, health check, backoff, persistent
  state, rolling upgrade, or user-selectable restart policy;
- a stable syscall, exception, task, scheduler, supervision, executable,
  Canticle, exec-hag, package, or control-plane ABI;
- more than one relationship, more than one restart, more than two live tasks,
  multiple tasks per circle, or recovery from an arbitrary exception;
- general user pointers, user-authored fault details, kernel events transported
  through ordinary IPC, or inheritance of sigils/endpoints/messages;
- SMP, APIC, NMI enablement, FPU/SIMD/TLS/debug-state switching, KPTI, DMA/IOMMU
  isolation, guarded privilege stacks, or microarchitectural isolation; or
- migration of the bootstrap shell or any production service out of ring 0.

## Security limitations

The kernel, bootstrap processor, firmware, emulator, fixed payloads, and all
ring-0 code remain trusted. The low identity-mapped kernel branch remains a
writable and executable supervisor alias in each task root. Privilege stacks
have allocator ownership and user-inaccessible mappings but no guard pages.
Only the exact fixture `#UD` is recoverable.

The nonlocal gate/exception return abandons Rust frames. Those frames must hold
no destructor, RAII guard, active exclusive borrow, or state that depends on
unwinding. The replacement checkpoint reduces but does not eliminate this
feature-only constraint.

An unexpected exception, gate, tick, identity, phase, reentry, sequence
exhaustion, clearing failure, or ownership inconsistency halts. This favors
privacy and non-reuse over availability and does not prove retry-safe recovery
from partial kernel failure.

## Acceptance evidence required

Architecture-neutral tests must cover all task/scheduler blocked transitions,
no-peer park, exact direct wake, queue topology, sequence exhaustion, stale and
wrong identities, failure atomicity, relationship capacity/generations,
lost-wake prevention, observation exactly-once consumption, restart-budget
exhaustion, new-incarnation binding, circle ownership, and record clearing.
Blocked sigil and IPC behavior must be explicit.

Host checks must mutate every fixed frame, exception, relationship, and
transcript predicate. The feature build must prove bounded payload sizes, no
relocations, exact linked symbols, and absence of forbidden private values.

`make supervision-smoke` must inspect the normal image first, build only the
`supervision_test` feature, derive the expected fault RIP from its ELF, boot
QEMU to debug-exit status 33, and accept only the frozen structured lifecycle.
`verify-supervision-transcript` must apply the same rules to a saved VirtualBox
log. Normal-image inspection must reject the complete feature event namespace,
roles, gate/payload/entry/runtime helpers, wait labels, and private marker byte
sequences while allowing reusable architecture-neutral mechanisms.

Acceptance additionally requires `make check`, every existing QEMU smoke and
normal-exclusion gate, the exact feature lifecycle in the dedicated VirtualBox
VM `Anomalous Dev [Threshold]`, and restoration of a normal interactive image
that remains running afterward.

## Acceptance

Accepted on 2026-09-13 after implementation checkpoint `3604345` and final
verification with:

- `cargo fmt --check`;
- `make check`;
- `make normal-exclusion`; and
- `make supervision-smoke`.

The accepted proof boots QEMU with the `supervision_test` feature, derives the
expected fault RIP from the linked ELF, rejects supervision-test payloads in the
normal image, and accepts only the frozen structured lifecycle. The transcript
proves one bounded restart: vessel generation 1 faults at the exact contained
`#UD`, the supervisor consumes one kernel-owned observation, authorizes one
restart at the safe kernel-stack checkpoint, vessel generation 1 storage and
identities are withdrawn, vessel generation 2 is created from cleared storage
and completes, the supervisor consumes completion, and final accounting returns
task, circle, scheduler, lease, and supervision state to zero.

A follow-up VirtualBox reproduction remains useful for local developer
confidence, but the ADR acceptance bar for the repository checkpoint is now met
by the automated QEMU and host verification above.

## Consequences

If accepted, Anomalous will gain the first reusable blocked-task and bounded
supervision state machines plus an end-to-end demonstration that a preserved
user-space supervisor can observe one contained failure and authorize a clean
replacement. That is a narrow foundation for future vessels, package-managed
services, the Canticle environment, and machine control without moving policy
or private context into the kernel mechanism.

The cost is another feature-only user execution path, additional task and
circle state, a single-use observation channel, two kernel continuations, and a
larger failure-atomic verification surface. None becomes a stable external ABI
through this decision.

## Alternatives considered

### Reset the faulted task to ready

This would preserve an address space, stack, bindings, and private residue from
the failed execution and would not prove generational replacement.

### Restart inside the exception handler

That would destroy or reuse resources while executing on a task privilege stack
and while the kernel allocator borrow is reachable only through abandoned Rust
frames.

### Use ordinary IPC for the fault observation

The existing IPC send path requires a running task and sigil. Forging either
for the kernel would blur provenance, and enqueueing ordinary IPC does not
define an authenticated wake transition.

### Add a general wait queue and restart policy now

Timeouts, cancellation, arbitrary events, backoff, dependency graphs, package
activation, and policy persistence are separable questions. One exact relation
and one restart isolate the mechanism that those later systems will need.
