# ADR 0015: Prove bounded preemption and user-fault containment

- Status: Accepted
- Date: 2026-09-09

This record accepts one deliberately fixed, feature-only scheduling experiment.
Its task count, virtual addresses, timer budget, gate vector, task roles, payload,
and event names are removable pre-alpha evidence, not a stable scheduler,
process, syscall, executable, or control-plane ABI.

## Context

ADR 0011 proved isolated CPL-3 entry, exact fault attribution, destructive
teardown, and clean replacement. ADR 0012 replaced ad hoc physical ownership
with bounded allocator leases and private address-space roots. ADR 0013 added
returning hardware interrupts and a monotonic PIT timebase. ADR 0014 then bound
allocator-backed tasks and circles to kernel-derived provenance, task-local
sigils, and copied IPC, but deliberately ran one task at a time with maskable
interrupts disabled.

The kernel therefore cannot yet demonstrate that an uncooperative user task can
be preempted, that two saved user contexts remain isolated, or that one exact
user fault can terminate only its task while a healthy peer continues. Moving
directly to a general scheduler would mix these mechanism questions with
priorities, fairness, blocking, multiprocessor synchronization, executable
loading, process policy, and a stable syscall surface.

One static TSS RSP0 stack is insufficient for preemption. A suspended user task
retains its processor return frame on the privilege-transition stack that
received its interrupt. Reusing that storage for a peer could overwrite the
only continuation capable of returning to the first task.

## Decision

### Scope

Add a `scheduler_test` feature and a bounded `make scheduler-smoke` proof on the
bootstrap processor. The proof creates exactly two one-task circles:
`faulting_fixture` and `peer_fixture`. Each owns a private address-space root,
one RX user code page, one RW+NX mailbox page, one RW+NX user stack page, and a
distinct allocator-backed four-page supervisor privilege stack.

Use a fixed round-robin architecture-neutral scheduler table. A real legacy PIT
IRQ0 provides exactly two preemptions. IRQ1 and every other external source are
masked for this experiment. The two tasks enter with IF clear and use one
feature-only, pointer-free DPL-3 ready/completion gate. The ready operation
permits the selected task to return with IF set only after its private state is
initialized. The completion operation is accepted only for the exact healthy
peer and returns through a single-use saved kernel continuation.

This decision introduces no new occult platform noun. `scheduler`, `task`,
`context`, and `fault` retain their precise conventional meanings; `circle` is
the already accepted isolation-domain term.

### Task lifecycle and scheduler model

Extend the bounded task lifecycle from `ready -> running -> exited` with the
states required by this proof:

```text
ready -> running -> suspended -> running -> faulted
ready -> running -> suspended -> running -> exited
```

A task owns at most one attached saved context. Dispatch requires `ready` or
`suspended` plus an attached context. Suspending or faulting a current task and
dispatching its peer is one checked composite state transition; failure leaves
both task records unchanged. Only `faulted` and `exited` tasks may withdraw a
context and be destroyed. A suspended task may retain installed authority and
an endpoint, but it cannot exercise authority until it is running again. New
sigil issuance, transfer, or attenuation targets accept only `ready` or
`running`; they reject `suspended`, `faulted`, and `exited`. IPC endpoints may
remain bound to suspended receivers, while authorized send or receive still
requires the acting task to be running.

The scheduler has a fixed backing maximum equal to the task-table capacity and
a configured active limit of two for this experiment. Its queue contains unique
task identities. Admission accepts an unattached `ready` task and atomically
attaches its initial context before publishing the scheduler record; no visible
entry is context-free. It rejects duplicate, nil, stale, or non-ready tasks.
Its only policy in this experiment is deterministic round robin. Counter or
sequence exhaustion is terminal and cannot wrap. This is not a claim of
fairness beyond the exact two-tick trace.

### Saved x86-64 context

Interrupt, exception, and synthesized initial user contexts use the same fixed
22-qword, 176-byte layout:

```text
rax rbx rcx rdx rbp rsi rdi r8 r9 r10 r11 r12 r13 r14 r15
vector error rip cs rflags rsp ss
```

Each initial frame resides exactly at `privilege_stack_top - 176`. Every saved
or selected frame must be naturally aligned, lie completely within the current
task's owned privilege-stack extent, have exact user selectors, reserved
RFLAGS bit one, IOPL zero, DF clear, and task-local RIP/RSP bounds. A dispatcher
may return a different validated frame pointer; the common assembly epilogue
pivots RSP to that frame, restores all general registers and user data
selectors, and executes `iretq`.

Kernel-origin interrupt frames remain the existing 20-qword shape. Dispatch
must require the complete 64-bit kernel CS value and must not inspect outer RSP
or SS unless an exact CPL-3 transition or designated IST frame supplied them.

Each task receives a distinct aligned 16 KiB privilege stack and the live TSS
RSP0 field is changed only while IF is clear and NMI remains masked. Because the
packed TSS field is not naturally aligned, access is bytewise volatile with
exact readback. The original bootstrap RSP0 value is restored before any task
stack is cleared or released.

CR0.TS remains set throughout all user execution and switching. The proof saves
no x87, MMX, SIMD, debug-register, FS/GS-base, PKRU, or CET state. The original
TS bit is restored only on the terminal kernel continuation. DS and ES are set
to the kernel selector in Rust handlers and to the user selector immediately
before a user return; FS and GS remain null in user mode.

### Exact execution trace

The bounded runtime performs this sequence:

1. allocate and validate both private roots, user pages, privilege stacks, task
   records, scheduler contexts, and the saved kernel continuation with IF clear;
2. select the faulting fixture's root, frame, and TSS RSP0, then enter user mode
   with IF clear;
3. accept its exact ready gate after private initialization and return with IF
   set;
4. on the next real PIT IRQ0, advance the monotonic timebase, acknowledge the
   master PIC once, save and suspend the faulting fixture, arm its private fault
   flag, and dispatch the peer's initial frame;
5. accept the peer's exact ready gate and return with IF set;
6. on the next real PIT IRQ0, save and suspend the peer, arm its private
   completion flag, and resume the faulting fixture;
7. after verifying its canaries and private flag, the faulting fixture executes
   the one expected `ud2` instruction;
8. contain only vector 6, error zero, the exact active task/root/frame/selectors,
   expected RIP, task-local state, and privilege-stack ownership; atomically
   fault that task and resume the peer;
9. the peer verifies its canaries, private state, and isolation before invoking
   the exact completion gate; and
10. abandon the peer stack, restore the kernel CR3, TSS RSP0, TS bit, and saved
    kernel stack, then withdraw and destructively clear allocator-owned task
    storage and address-bearing runtime bookkeeping in dependency order.

The wrong user branch, unexpected gate, extra tick, wrong task, wrong root,
malformed frame, unexpected exception, reentry, or failed teardown is fatal.
Neither IRQ nor exception code formats serial output while scheduling is live;
it records bounded observations and emits the transcript only after returning
to the kernel stack with IF clear.

### Ownership and teardown

Each task owns three data frames through its user mappings, four page-table
frames through its private root, and four data frames through its privilege
stack. The ready-state delta for two tasks is therefore exactly 14 data frames,
8 table frames, and 22 total free frames. The privilege-stack lease is bound
for the complete attached-context lifetime and represented in circle child
accounting.

Teardown restores the kernel CR3 and bootstrap TSS RSP0 before clearing any
task stack. It then withdraws scheduler entries and contexts, removes user
mappings and roots, clears and releases all user and privilege-stack frames,
destroys terminal tasks, and finally destroys their empty circles. Every frame,
table, lease, bound-lease, task, circle, context, mapping, and queue count must
equal its initial value. A task's current privilege stack is never cleared while
code is executing on it.

That complete reclamation claim applies only to the successful 25-record
lifecycle. Any setup or teardown inconsistency is terminal. Storage whose
ownership has not been conclusively withdrawn remains owned and quarantined
until halt; this experiment does not implement rollback, retry, or recovery
from partial setup or teardown.

### Observable lifecycle

The strict proof transcript consists of exactly these 25 structured records:

1. `boot.kernel.enter`
2. `kernel.fault_path.ready`
3. `boot.omen.accepted`
4. `boot.memory.map`
5. `kernel.memory.ready`
6. `boot.display.ready`
7. `boot.ready`
8. `kernel.interrupts.ready`
9. `kernel.scheduler_test.ready`
10. `kernel.scheduler_test.task.ready` for `faulting_fixture`
11. `kernel.scheduler_test.task.ready` for `peer_fixture`
12. `kernel.scheduler_test.task.dispatched` for the initial faulting task
13. `kernel.scheduler_test.task.preempted` for the faulting task
14. `kernel.scheduler_test.task.dispatched` for the initial peer task
15. `kernel.scheduler_test.task.preempted` for the peer task
16. `kernel.scheduler_test.task.resumed` for the faulting task
17. `kernel.exception` for the exact contained user `#UD`
18. `kernel.scheduler_test.task.faulted` for the faulting task
19. `kernel.scheduler_test.task.resumed` for the peer task
20. `kernel.scheduler_test.task.completed` for the peer task
21. `kernel.scheduler_test.interrupts.masked`
22. `kernel.scheduler_test.task.withdrawn` for the faulting task
23. `kernel.scheduler_test.task.withdrawn` for the peer task
24. `kernel.scheduler_test.released`
25. `kernel.scheduler_test.passed`

Only static roles, generations, aggregate ownership, transition sequences,
ticks, permission labels, lifecycle states, booleans, and the fixed non-secret
RIP/RSP fields required by the standard contained `kernel.exception` record may
be reported. That record reports a null `fault_address`. The transcript never
emits physical addresses, arbitrary task addresses, frame pointers, general
registers, canaries, fault flags, private markers, user payloads, or arbitrary
user bytes.

## Required invariants

- Only a real armed PIT IRQ0 may preempt a task in this proof.
- The scheduler never trusts a user-supplied task, circle, context, root, stack,
  identity, or policy field.
- Every selected frame, CR3, and TSS RSP0 belongs to the same live scheduled
  task and is revalidated before return.
- A saved frame never outlives its bound privilege-stack lease.
- A context switch is all-or-nothing across task state and scheduler state.
- The expected `#UD` faults only the exact selected task; every other exception
  remains terminal.
- The peer executes after the contained fault and proves private-state
  continuity without reading the faulting task's private pages.
- TS, selectors, IF, CR3, and TSS state are restored to their declared terminal
  values before feature storage is reclaimed.
- Every allocator-owned user, page-table, and privilege-stack byte is cleared;
  inactive ownership/address metadata is scrubbed; and every accounting value
  is restored on the successful lifecycle.
- Serial diagnostics remain output-only and disclose no private task state.

## Non-goals

This decision does not define or implement:

- a production scheduler, fairness guarantee, priorities, deadlines, quantum
  policy, blocking, wakeup, cancellation, work stealing, or load balancing;
- SMP, AP startup, APIC, x2APIC, NMI enablement, cross-CPU synchronization, or
  migration;
- a general process, thread, vessel, supervisor, service, or restart model;
- arbitrary executable loading, more than two tasks, multiple tasks per circle,
  fork, spawn, wait, or parent/child policy;
- a stable syscall, scheduling, task, context, signal, fault, or executable ABI;
- general user pointers, buffers, IPC, rites, Canticle, exec-hag, or AI policy;
- general recoverable exceptions, page-fault fixup, signals, user exception
  handlers, or denial-of-service containment beyond the exact PIT path;
- FPU/SIMD ownership, lazy/eager extended-state switching, debug state, TLS,
  FS/GS bases, PKRU, CET, or hardware performance state; or
- DMA/IOMMU isolation, guarded kernel stacks, KPTI, ASLR, SMEP, SMAP, speculative
  channel resistance, or production security.

## Security limitations

The bootstrap processor, kernel, firmware, emulator, fixed payloads, and all
ring-0 code remain trusted. The low identity-mapped kernel branch remains a
writable and executable supervisor alias in both task roots. Privilege stacks
have allocator ownership and user-inaccessible mappings but no guard pages.
There is no DMA or microarchitectural isolation.

The proof's exact `#UD` recovery is a test predicate, not general fault
containment. A task that disables progress through an unmodeled hardware path,
causes a different exception, corrupts the kernel through an unknown kernel
bug, or exploits shared hardware state remains outside the claim. NMI is masked
and only the BSP runs, so bytewise TSS updates do not establish an SMP-safe TSS
policy.

The terminal peer-completion path deliberately resumes a saved kernel stack
without unwinding the abandoned Rust interrupt-gate frames. Those frames hold
no destructors, RAII guards, or lasting borrows; preserving that invariant is a
requirement until this feature-only continuation is replaced. Kernel interrupt
entry also reloads FS and GS under the current base-zero, no-TLS model. The
experiment does not establish preservation for future TLS, per-CPU GS, or other
extended processor state.

Setup and teardown failures halt rather than attempt recovery. Any allocation
still recorded as owned remains quarantined, so the failure path favors
non-reuse over availability but does not prove leak-free rollback.

The scheduler roles are kernel-authored test labels, not authenticated human,
machine, AI, or service identities. ADR 0014 already established equivalent
human/machine mechanism; this proof is deliberately role-neutral and makes no
new semantic-control claim.

## Acceptance evidence

Architecture-neutral tests cover scheduler capacity, empty
selection, duplicate/stale/nil identities, invalid states, atomic failure,
round-robin order, counter exhaustion, current-task rules, fault removal,
completion, context attachment/withdrawal, and destructive task cleanup. Frame
layout, pointer bounds, selector/flags gates, TSS RSP0 byte updates, and
one-field mutations of every expected-fault predicate also have host tests.

`make scheduler-smoke` builds and inspects the normal image first, boots the
feature under QEMU, requires debug-exit status 33, and accepts only the exact
25-record lifecycle. Its verifier rejects missing, duplicate, reordered,
unknown, malformed, unstructured, error, fatal, address-, pointer-, register-,
canary-, payload-, or private-shaped content outside the fixed exception RIP
and RSP. Tick values may vary but must be strictly increasing and the final
summary is derived from the preceding records.

Normal-image inspection must reject every feature gate, user payload entry,
context trampoline, fixture label, private marker, event namespace, and test
hook while allowing reusable architecture-neutral scheduler mechanisms.
Acceptance additionally requires `make check`, every existing QEMU regression,
the same exact feature lifecycle in the dedicated VirtualBox VM
`Anomalous Dev [Threshold]`, and restoration of a normal interactive image that
remains running afterward.

The implementation checkpoint is `8e0f85d`. On 2026-09-09, `make check`
passed 3 `anomalous-abi` tests, 214 base kernel tests, 220 kernel tests with
`scheduler_test`, 26 `xtask` tests, target and host Clippy with warnings denied,
format checking, and shell-script syntax checks. The complete QEMU regression
matrix—normal, fault, circle, control, memory, interrupt, shell, and normal-image
exclusion—also passed.

The strict QEMU scheduler run accepted exactly 25 records, observed PIT ticks 1
and 2, contained the fixed `#UD` at RIP `0x00007000000010a5`, resumed the peer,
and restored all 65,112 initial free frames and every feature accounting value.
The exact 512 MiB VirtualBox VM reproduced the 25-record lifecycle at PIT ticks
2 and 3 and restored all 130,665 initial free frames. Its exception RFLAGS
differed from QEMU only by the architecturally permitted Resume Flag (`RF`);
the exact matcher accepted that declared variance. Neither trace contained an
error, fatal record, or private marker.

After acceptance, the normal image was rebuilt and launched in the same
VirtualBox VM. It reported `boot.ready` and `bootstrap_shell.ready` with 130,683
free frames and no error or fatal record, and it remains running. These results
accept only the bounded mechanism and successful cleanup described here; they
do not convert the experiment into a production scheduler or general recovery
facility.

## Consequences

Anomalous gains a small reusable scheduler state
machine plus its first end-to-end proof that real timer preemption, private saved
contexts, address-space switches, and one exact user-fault containment path can
compose without losing ownership or peer progress. The mechanism remains useful
to both human- and machine-directed workloads without embedding either client
class into low-level scheduling policy.

The cost is a second user-return path in common interrupt and exception
assembly, per-task kernel-stack ownership, more task states, a feature-only gate
and payload, and a much stricter teardown and evidence surface. None becomes a
stable external ABI through this decision.

## Alternatives considered

### Reuse the static TSS privilege stack

This would let the peer's next privilege transition overwrite a suspended
task's only saved return frame.

### Use a software `int` as the scheduling tick

That proves a cooperative trap, not external preemption of an uncooperative
task. The experiment therefore requires the already owned real PIT IRQ0 path.

### Recover every user exception

General fault recovery needs signal policy, hostile-pointer rules, nested-fault
containment, resource supervision, and an external ABI. This slice admits only
one exact fixture `#UD` predicate.

### Add priorities, blocking, or multiple tasks per circle

Those policies multiply queue and ownership states before the context-switch
mechanism is independently established. Two one-task circles are the narrowest
test of isolation across preemption and fault.
