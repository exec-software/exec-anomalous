# ADR 0039: Prove bounded sustained native dispatch

- Status: Proposed
- Date: 2026-09-26

## Context

The native desktop can render caller-owned content inside windows. The user now
wants to return to kernel foundations before developing a modular native
execution boundary and Linux-first compatibility. Accepted
[ADR 0015](0015-prove-bounded-preemption-and-user-fault-containment.md) proves two
PIT preemptions and one exact task-local fault. It does not exercise sustained
switching or continued timer delivery when only one runnable task remains.

The native rite prefixes are implemented experimental contracts, not a stable
application ABI. Reliquary remains a proposed executable/package design, and no
runtime Linux ELF loader or foreign syscall personality is implemented. The
[execution-readiness audit](../architecture/execution-readiness.md) records
these distinctions and the remaining prerequisites. This increment must not
make a foreign executable format or syscall table part of the kernel.

## Decision

Add a separate removable `dispatcher_test` feature and `make dispatcher-smoke`
proof on the bootstrap processor. Keep the established scheduler proof and
interactive workspace behavior unchanged. Reuse the existing bounded task,
circle, scheduler, owned-address-space, saved-frame, and privilege-stack
mechanisms rather than introduce production scheduling policy.

The proof has exactly two private GPR-only tasks. Each owns a distinct root,
RX code, RW+NX data/mailbox and stack mappings, and a separate supervisor
transition stack. Fixed task-local canaries in GPRs, a private stack slot, and
the private mailbox plus an integer progress counter distinguish the tasks.
A real PIT interrupt supplies each of 64 task switches; a
synthetic loop or voluntary yield is not evidence of hardware preemption.
Every selected context must preserve the exact task identity, private root,
owned transition stack, user selectors, permitted flags, and task-local
instruction and stack bounds.

After the two-task switching phase, the designated fixture executes one known
`ud2`. Containment requires the complete expected task, vector, error, instruction
address, root, frame, and private-state predicate. Unexpected exceptions and
malformed contexts still fail closed; this is not generic fault recovery.

The healthy peer then runs through 16 further real PIT ticks as the sole
runnable task. Its user counter must demonstrate progress during this phase.
With no peer to select, the interrupt returns to the validated current frame
without suspending and redispatching that task merely to manufacture another
dispatch. Timer observations and actual dispatch sequence have separate
meanings: the timebase advances, while the dispatch sequence remains unchanged
until a real scheduling transition occurs.

The expected totals are 80 PIT ticks and 66 dispatches: the initial dispatch,
64 timer-driven switches, and the dispatch to the survivor after the fault.
The sole-task phase adds no dispatch or preemption sequence. The two task
spaces retain the existing bounded ownership shape of 14 data frames and
eight page-table frames in total. Reuse the established scheduler proof's
frame and task-space allocation, validation, and release helpers without
altering that proof's execution sequence.

Completion uses one exact feature-only gate and a single-use saved kernel
continuation. Restore the kernel CR3, original TSS RSP0, and pre-entry CR0.TS
state before withdrawing task contexts or clearing transition stacks. Remove
task, circle, mapping, scheduler, and allocator ownership in dependency order;
verify private storage and address-bearing runtime state are cleared and all
initial free frames are restored. A failed clear cannot authorize reuse or a
successful-completion record.

The proof remains BSP-only with the existing legacy PIC/PIT backend and NMI
masked. User execution keeps CR0.TS set; no x87, MMX, SIMD, FS/GS-base, TLS,
debug-register, PKRU, or CET context is claimed. IRQ1 and unrelated sources
remain masked. No serial formatting occurs in the live switching path;
bounded observations are emitted only after reaching the safe kernel
continuation. Public evidence contains lifecycle, counts, identities already
approved for the fixture, and progress predicates, not private mailbox bytes,
raw registers, or arbitrary addresses.

## Validation and disproof

The native runtime derives the controlled fault address from its linked
fixture symbol and requires the exact relocated instruction address. The
address is not published in serial telemetry. `dispatcher-smoke` accepts only
the complete ordered transcript of the resulting native predicates; its host
oracle does not independently inspect that fault address. Require 64 real
two-task PIT switches, exactly one contained known fault, 16 subsequent
sole-runnable timer ticks, measured user progress, unchanged sole-task dispatch
sequence, terminal completion, and exact resource restoration. Test transcript
mutations that remove, duplicate, reorder, or alter phase counts, progress,
fault attribution, and teardown evidence. Host scheduler tests must cover the
no-peer path and prove that failure does not mutate task or scheduling state.
The feature's six aggregate records are `ready`, `progress`, `fault_contained`,
`survivor`, `released`, and `passed` under the `kernel.dispatcher_test` namespace.
Their fixed ordering and complete schema are part of the removable proof,
not a stable event ABI.

Run the repository's full required validation matrix, the retained desktop and
workspace proofs, and normal-image exclusion. The ordinary image must contain
no dispatcher feature namespace, fixture symbols, private marker, or feature
payload. Record actual commands and outcomes in the
[Sustained Native Dispatch Log](../../notebook/Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md).
VirtualBox evidence is required when the available host supports that run;
an unavailable backend must be reported, not silently counted as passing.

The proposal is disproved by lost or aliased private state, stale context
selection, a manufactured dispatch with no peer, missing user progress,
unexpected fault recovery, incomplete clearing, a leaked feature in the normal
image, or any regression in prior proofs. Final TCG and KVM runs both passed
the strict 14-record transcript and restored 65,112 free frames. All 23
validation targets passed, including the complete ordinary regression matrix,
normal-image exclusion, and separate KVM dispatcher/workspace repeats, on
unchanged frozen source. A separate isolated VirtualBox attempt failed before
guest execution with `VERR_SVM_IN_USE` because KVM owns AMD-V; its private VM
and disk were cleaned up, and no existing VM or host setting was changed.
The development log records commands, durations, image identities, and limits.
This record remains Proposed.

## Consequences and limits

The experiment extends measured confidence in existing scheduling mechanisms
and closes a specific sole-runnable-task gap. It does not complete kernel
foundations or establish fairness, real-time bounds, SMP, dynamic admission,
general blocking/wake, production service supervision, FPU/TLS preservation,
general user-copy, or arbitrary application faults. It does not change the
desktop into a multiprocess application environment.

Future native launch work still needs lower-address user mappings, more general
segment geometry, explicit execution-state and resource profiles, failure-atomic
admission, and independently validated authority. A versioned native launch
contract and Reliquary execution framing precede a separate Linux ELF adapter
and Linux syscall personality. macOS later uses an independent Mach-O/Darwin
veil. Rich artifact parsers and foreign-platform policy remain outside the
kernel under [ADR 0008](0008-isolate-foreign-platform-compatibility.md).
