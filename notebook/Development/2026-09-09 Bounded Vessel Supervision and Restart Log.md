---
title: "Bounded Vessel Supervision and Restart Log"
date: 2026-09-09
tags: [development, kernel, scheduler, vessel, supervision, restart]
status: active
domain: operating-system
---

# Bounded Vessel Supervision and Restart Log

## Goal

Prove the smallest honest supervision boundary after bounded scheduling: a
persistent user task blocks for a kernel-owned observation, survives the exact
fault of one vessel generation, authorizes one destructive replacement at a
safe kernel-stack checkpoint, and resumes after the clean replacement exits.

This is foundation for eventually moving restartable services out of ring 0.
It is not a production vessel runtime, service manifest, package lifecycle, or
stable ABI.

## Proposed contract

**Proposed:** ADR 0018 defines a fixed `supervision_test` experiment with one
`supervisor_fixture`, one `vessel_fixture`, one real PIT preemption, one exact
contained `#UD`, one restart budget, and a second fresh task/circle generation.

**Proposed:** `blocked` is a live but non-runnable task state. Only a checked
supervision composite may directly wake the exact recorded supervisor. A
single-slot, kernel-authored, coarse observation prevents forged provenance and
lost wakes.

**Proposed:** destructive replacement occurs only after the supervisor parks
with no runnable peer and the common assembly path restores kernel CR3,
bootstrap TSS RSP0, TS, and the original kernel stack. The frame allocator is
never published through the task runtime and no active task stack is erased.

**Proposed:** the failed task and circle are destroyed and their identities
become stale. The replacement receives new generations and cleared storage;
the persistent supervisor retains its own saved context and private state.

## Review findings

- The current 176-byte scheduler frame and interrupt/exception return epilogues
  are sufficient; a second context layout would add risk without evidence.
- A blocked context must remain attached to its privilege-stack lease while
  being absent from current and runnable queue state.
- Fault-and-wake and completion-and-wake must be atomic scheduler/task
  composites. Composing independent wake and fault calls would expose partial
  state on dispatch-sequence exhaustion or corruption.
- The supervisor circle should own the supervision relationship. The disposable
  vessel circle remains a validated referent so the existing one-task circle
  destruction invariant stays intact.
- Existing sigil checks already prevent blocked tasks from exercising or
  receiving new authority. IPC may keep a blocked receiver bound, but enqueue
  does not wake it in this slice.
- Replacement construction and teardown need explicit ownership/quarantine
  rules. A partial erase or release must halt without publishing success or
  reusing ambiguous storage.

## Intended evidence

| Layer | Required observation |
| --- | --- |
| Architecture-neutral | Failure-atomic block, park, fault-wake, completion-wake, observation, restart-budget, generation, circle-child, and clearing tests |
| x86-64 | Real CPL-3 block/resume, one PIT/EOI, exact `#UD`, exception-frame pivot to supervisor, two kernel checkpoints, fresh generation, and final reclamation |
| Privacy | Strict structured transcript contains no roots, frames, pointers, registers, canaries, mailbox data, payloads, or user-provided fault text |
| Exclusion | Normal image contains no supervision feature namespace, roles, gates, payloads, runtime hooks, wait labels, or private marker bytes |
| Regression | All existing host, QEMU, shell, scheduler, and normal-image gates continue to pass |
| VirtualBox | Dedicated VM reproduces the exact frozen lifecycle; normal interactive image is restored afterward |

## Open during proposal

- Decide the smallest clean sharing boundary for the accepted scheduler entry
  assembly so `scheduler_test` and `supervision_test` compile together under
  `--all-features` without either runner depending on the other.
- Do not call this a general vessel or supervisor ABI until arbitrary programs,
  authenticated control, cancellation, policy persistence, and package
  lifecycle are independently designed and tested.

## Evidence log

### 2026-09-13 — Accepted repository proof

Implementation checkpoint `3604345` accepted ADR 0018. The proof adds the
architecture-neutral supervision table, blocked/wake/replacement mechanics, the
x86-64 `supervision_test` fixture, and xtask transcript verification.

Commands run and passed:

- `cargo fmt --check`
- `make check`
- `make normal-exclusion`
- `make supervision-smoke`

The QEMU transcript proved the fixed lifecycle: initial vessel dispatch, one real
PIT preemption, supervisor block, exact vessel `#UD`, single-use fault
observation, supervisor restart checkpoint, destructive withdrawal of vessel
generation 1, clean generation 2 creation, completion observation, supervisor
completion, and final reclamation to zero active scheduler/task/circle/lease and
supervision state. Normal-image inspection rejected the supervision feature
namespace, fixture symbols, and private marker bytes.

Debugging notes from the acceptance pass:

- The supervision fixture needed a public in-bounds ready-return symbol for the
  first vessel gate and a distinct in-bounds supervisor-complete return symbol
  instead of treating one-past-end as an executable return address.
- The supervision record validator had to permit the valid `replacing_incarnation`
  phase where no active incarnation exists yet; the same-task guard now applies
  only when an active incarnation is present.
- Supervision-only smoke builds require the same `monotonic_instant`
  tick/timebase accessors used by the scheduler and interrupt proof features.
- Gate frames intentionally carry the fixture syscall convention with IF clear;
  timer/exception paths remain responsible for the IF-enabled user-interrupt
  evidence.
