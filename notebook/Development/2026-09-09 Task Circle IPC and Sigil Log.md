---
title: "Task, Circle, IPC, and Sigil Log"
date: 2026-09-09
tags: [kernel, circle, task, ipc, sigil, ring3, authority]
status: accepted
domain: foundations
---

# Task, Circle, IPC, and Sigil Log

## Question and result

Can the current allocator and fixed ring-3 machinery support one genuinely
kernel-attributed, task-local-authority message boundary without prematurely
inventing a scheduler, process ABI, or semantic control plane?

**Observed and accepted:** fixed-capacity state machines plus one feature-only
x86-64 gate prove the bounded core invariant. A one-shot task can use only
authority installed in its own namespace; the kernel—not caller data—attributes
its copied message; sender teardown cannot mutate that message; and a receiver
can consume it exactly once before every resource is invalidated and cleared.
The implementation checkpoint `7663420` was committed and pushed before this
acceptance record.

## Accepted boundary

[ADR 0014](../../docs/adr/0014-prove-fixed-attributable-sigil-gated-ipc.md)
defines the accepted experiment. The observed runtime sequence is:

1. prepare an allocator-backed receiver circle, task, two-slot endpoint, and
   receive-only local sigil binding;
2. run one fixed producer payload under a kernel-selected synthetic human role,
   reject an invalid binding, accept one send, return to ring 3, then exit and
   destructively tear down its circle;
3. recreate the producer with fresh generations and a synthetic machine role,
   run byte-identical code through equivalent authority, and enqueue sequence
   one without altering sequence zero;
4. enter the receiver, reject copied producer binding bits, receive both fixed
   records FIFO, observe distinct kernel provenance and equal payloads, then
   receive an explicit empty result;
5. revoke authority, destroy the endpoint and tasks, withdraw both roots, clear
   all private storage, and restore exact allocator accounting; and
6. emit only aggregate structured evidence and exit through the feature-only
   QEMU path.

## Security decisions

- `anomalous_abi::invocation_header` remains caller-controlled layout
  scaffolding and is not used as identity or bearer authority in this proof.
- A sigil reference is task-local `{slot, generation, tag}` data resolved through
  a kernel-owned binding table. Copying its bits does not transfer the binding.
- The kernel stamps source task, circle, role, and fixture principal; user state
  cannot supply those authoritative fields.
- IPC uses a distinct fixed queue with explicit backpressure, not the input event
  channel's deliberate drop-and-loss behavior.
- The gate accepts no arbitrary pointer. It accesses only the active circle's
  fixed, prevalidated mailbox through an allocator-authorized physical alias.
- Ring-3 runs with `IF=0` and `CR0.TS` set. The single TSS privilege stack cannot
  retain a blocked continuation, so every task is finite and cooperative.
- Synthetic human and machine roles prove equivalent mechanisms and distinct
  provenance only. They do not authenticate a person, model, or agent.

## Acceptance evidence

| Boundary | Observed evidence |
| --- | --- |
| Host and target checks | `make check` passed 3 `anomalous-abi` tests, 193 kernel tests, 22 `xtask` tests, and target Clippy with warnings denied |
| QEMU regression | Boot, fault, circle, control, memory, interrupt, and shell smoke targets all passed; normal-image exclusion also passed |
| QEMU control proof | The closed-world verifier accepted exactly 28 structured lifecycle records and exact restoration of 65,114 free frames |
| VirtualBox control proof | `Anomalous Dev [Threshold]` reproduced the exact 28 records and restored its initial 130,667 free frames |
| Normal restoration | The normal image was rebuilt, restored, and observed running at `bootstrap_shell.ready` with 130,683 free frames and no error or fatal record |

The implementation connects fixed circle and task lifecycle records,
allocator-backed RX and RW+NX mappings with absent guards, task-local sigil
bindings, one receiver-owned copied-message queue, and real CPL-3 entry. The
human and machine producer fixtures execute byte-identical code through the
same composite authorized send path. The receiver's copied reference is denied,
both immutable messages arrive FIFO with kernel-derived attribution, empty is
reported exactly, revocation takes effect, and all owned state is withdrawn and
cleared before allocator counts return to their starting value.

Independent final audit closed a structural authorization gap by making raw
queue operations module-private and exposing composite authorized operations.
It also required validation of every occupied queue record, fail-closed sigil
allocation, a canonical maximum next-sequence value for exhausted endpoints,
and verifier-derived receiver and final accounting instead of trusting
self-reported summaries. QEMU and VirtualBox evidence was generated from the
hardened tree.

## Explicit limits

This accepted proof does not claim scheduling, preemption, blocking, safe
arbitrary user buffers, stable syscall or IPC ABI, general processes or vessels,
authenticated identity, semantic rites, policy, idempotency, Canticle, exec-hag,
user-mode interrupts, arbitrary fault recovery, DMA isolation, or production
security.

## Observations

ADR 0014 is accepted only for this fixed feature. Synthetic human and machine
roles establish equivalent mechanism and distinct kernel attribution; they do
not authenticate a person, model, agent, or external client. No scheduler,
stable ABI, or general vessel runtime follows from the result.
