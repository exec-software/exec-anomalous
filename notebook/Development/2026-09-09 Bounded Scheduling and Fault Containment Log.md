---
title: "Bounded Scheduling and Fault Containment Log"
date: 2026-09-09
tags: [kernel, scheduler, x86-64, preemption, fault-containment]
status: accepted
domain: operating-system
---

# Bounded Scheduling and Fault Containment Log

## Acceptance checkpoint

**Observed and accepted:** ADR 0015 bounds the experiment to two
allocator-backed, one-task circles. A real PIT IRQ0 preempted each task once,
the first task resumed and executed one exact `ud2`, and the exception path
faulted only that task before the healthy peer resumed and completed.

The design does not reuse the static TSS RSP0 stack for suspended contexts.
Each task owns a distinct aligned four-page supervisor privilege stack and one
fixed 176-byte saved user frame. The scheduler must keep the task identity,
address-space root, frame extent, and selected TSS RSP0 inseparable at every
return boundary.

The fixed ownership target is:

| Resource | Per task | Two-task proof |
| --- | ---: | ---: |
| User data frames: RX code, RW+NX mailbox, RW+NX stack | 3 | 6 |
| Supervisor privilege-stack frames | 4 | 8 |
| Private page-table frames | 4 | 8 |
| Total free-frame delta | 11 | 22 |

No new occult noun is proposed. This is conventional scheduler mechanism under
the already accepted task and circle vocabulary. The low-level policy remains
neutral to whether a later workload is directed by a person, AI, or another
authorized client.

## Implementation

The pushed implementation checkpoint is `8e0f85d`; the proposal checkpoint is
`488a68f`. The reusable architecture-neutral scheduler owns a fixed queue of
generational task identities and composes checked task-table transitions for
dispatch, preemption, fault, completion, context attachment, and withdrawal.
Ready, running, suspended, faulted, and exited are explicit states. Suspended
tasks retain installed state but cannot exercise sigil authority until they run
again.

The x86-64 `scheduler_test` feature gives each task one private CR3, three user
pages, four page-table frames, and a distinct four-page privilege stack. The
interrupt and exception paths share an exact 22-qword user context. Before each
return, the kernel revalidates the task, root, frame extent, selectors, flags,
RIP/RSP bounds, TSS RSP0 ownership, and `CR0.TS` policy. Exactly two armed PIT
IRQs drive the round-robin switch; all other external sources are masked.

## Review hardening

Independent ABI, runtime, and harness reviews drove the final hardening pass:

- scheduler runtime host tests now compile under `scheduler_test` and mutate
  every expected-fault snapshot and overlapping saved-frame field;
- the exception path cross-checks the bounded observation against the live
  frame, and every unexpected scheduler-active exception emits only a static
  terminal reason rather than attacker-selected register or address state;
- normal-image exclusion covers the feature gate, assembly returns, payloads,
  fixtures, markers, namespace, and test hooks;
- the transcript verifier covers missing, duplicate, reordered, malformed,
  unknown, failed, private, and one-field-mutated evidence;
- successful teardown scrubs released allocation addresses, retired lease
  metadata, borrowed-root metadata, scheduler task records, and runtime
  address-bearing state; and
- initial mailboxes must be zero before payload initialization, while initial
  frames and resumed frames apply their correct, distinct `CR0.TS` predicates.

The nonlocal peer-completion continuation intentionally abandons Rust gate
frames. Those frames contain no destructors, RAII guards, or lasting borrows.
FS/GS handling is also scoped to the present base-zero, no-TLS model. Both are
explicit constraints on future evolution, not general context-switch claims.

## Observed evidence

- `make check` passed 3 `anomalous-abi` tests, 214 base kernel tests, 220 kernel
  tests with `scheduler_test`, 26 `xtask` tests, formatting, shell syntax, and
  host and target Clippy with warnings denied.
- `make scheduler-smoke` accepted exactly 25 QEMU records with debug-exit status
  33. PIT ticks were 1 and 2; the contained `#UD` RIP was
  `0x00007000000010a5`; the peer continued; and all 65,112 initial free frames
  plus task, circle, context, queue, lease, bound-lease, and mapping counts were
  restored.
- The normal, fault, circle, control, memory, interrupt, and shell QEMU targets
  plus normal-exclusion image inspection all passed after scheduler hardening.
- The exact dedicated 512 MiB VirtualBox VM `Anomalous Dev [Threshold]`
  reproduced the 25-record trace at PIT ticks 2 and 3 and restored all 130,665
  initial free frames. Its exception RFLAGS differed from QEMU only by the
  declared Resume Flag (`RF`) variance.
- The normal image was rebuilt and launched afterward. It emitted `boot.ready`
  and `bootstrap_shell.ready` with 130,683 free frames and no error or fatal
  record, and remains running. The strict feature trace is preserved at
  `build/virtualbox-scheduler.log`; normal boot evidence is at
  `build/virtualbox.log`.

## Claim boundary

This work must not be recorded as a general scheduler, production fault
recovery, process or vessel model, syscall ABI, FPU/SIMD context switch, SMP
implementation, fairness guarantee, or authenticated human/AI integration.
Tick values may vary between emulators, but the two-preemption order, exact
contained fault, peer continuation, ownership delta, cleanup, and record count
may not.

Complete cleanup is proven only by the successful 25-record lifecycle. A setup
or teardown inconsistency is terminal; any allocation whose withdrawal is not
conclusive remains owned and quarantined until halt. This slice claims neither
rollback nor retry-safe cleanup on those failure paths.

## Results

ADR 0015 is Accepted. The fixed proof demonstrates that real timer preemption,
two private saved contexts, address-space and TSS-stack switching, exact
single-task faulting, peer continuation, and successful destructive teardown
compose on the BSP. General scheduling policy, arbitrary task admission,
blocking, supervision, authenticated identities, stable user ABI, and general
fault recovery remain open.
