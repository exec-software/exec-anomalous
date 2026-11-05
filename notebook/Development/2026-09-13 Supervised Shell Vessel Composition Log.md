---
title: "Supervised Shell Vessel Composition Log"
date: 2026-09-13
tags: [development, kernel, shell, vessels, scheduler, supervision, rites, x86-64]
status: accepted
domain: operating-system
---

# Supervised Shell Vessel Composition Log

## Goal

Compose the accepted scheduler, one-restart supervision relationship, and
native x86-64 rite admission path around one fixed ring-3 shell-vessel fixture.
The experiment should also test the architectural rule that a human-readable
projection and a machine-direct projection reach the same low-level semantic
request and policy result.

This is not yet a user-space shell, Canticle, exec-hag, physical input path,
stable syscall ABI, service runtime, display manager, or package lifecycle.

## Proposed slice

**Proposed:** ADR 0020 adds a removable `shell_vessel_test` image. The first
incarnation projects one compiled exact `echo` form into `console_write`, polls
empty input, is preempted once by the PIT, and faults through the accepted
contained `#UD` path. Its supervisor consumes the fault observation and reaches
the safe restart checkpoint.

**Proposed:** only after the original kernel stack and processor state are
restored does the kernel retire the first rite channel and destructively clear
the failed incarnation. The replacement receives fresh task/circle identities,
cleared storage, the same minimal rights, and rite sequence one.

**Proposed:** the replacement models a machine client by constructing the same
console tuple directly. Equality includes the operation, payload kind, declared
right, size bounds, body, and result while preserving distinct private trace
identity. Serial reports only a static projection label and
`semantic_equal=true`.

**Proposed:** zero-status `task_exit` is terminal. It must settle rite admission,
complete the scheduled replacement, publish `successful_exit`, and wake the
supervisor atomically. No reply is copied and no `iretq` targets the exited
vessel; the rite entry selects the supervisor's saved frame.

## Initial design findings

- Keep `shell_vessel_test` independent from the terminal proof features and
  widen reusable feature gates explicitly. This prevents an all-features build
  from accidentally running two terminal harnesses.
- Reuse the scheduler's user-window, saved-frame, task-space, and enter/resume
  machinery and the supervision harness's two-session kernel checkpoint.
- Extract the fixed rite mailbox copying and clearing helpers rather than
  duplicating unsafe transport code.
- Reserve the entire mailbox page for valid rite envelopes. Put the post-PIT
  fixture marker in unused private stack storage and wipe that page at
  withdrawal.
- The current input rite can honestly prove only empty `would_block` behavior.
  A later decision must design nonempty typed delivery and blocking before a
  real interactive shell vessel is claimed.
- Strict evidence must exclude the source line, human-visible output, trace
  values, mailbox bytes, addresses, registers, and private markers from COM1
  and from the ordinary kernel image.

## Intended evidence

| Layer | Required observation |
| --- | --- |
| Architecture-neutral | Existing scheduler, supervision, task, circle, and rite invariants remain unchanged; any extracted transport has exact layout and bounds tests. |
| x86-64 | Both incarnations invoke native rites while scheduled; a real PIT, contained fault, safe restart, fresh channel, terminal supervisor wake, and complete teardown occur in one image. |
| Coequal clients | Fixed human-source and machine-direct projections produce the same console semantic tuple and result without equating their attribution. |
| Privacy | A closed-world structured transcript contains no source, output, trace, mailbox content, pointers, roots, physical addresses, registers, or canaries. |
| Exclusion | The normal image contains no feature namespace, gate/fixture symbols, source/output bytes, or private marker. |
| Regression | Every accepted host and QEMU proof remains green, including the standalone rite and supervision images. |

## Open during proposal

- Freeze the exact closed-world record schema before implementing the verifier
  so runtime events and mutation tests share one explicit contract.
- Re-audit every nonlocal Rust frame on terminal exit: no abandoned frame may
  own a destructor, live exclusive borrow, or state that depends on unwinding.
- Keep the fixed projection claim narrow. It is evidence for one shared
  mechanism, not evidence that a shell grammar or canonical machine encoding
  has been designed.

## Implemented composition

**Decided:** ADR 0020 is accepted for one removable, fixed x86-64 proof. The
implementation checkpoint is `8da910a`.

The shared rite transport now owns explicit little-endian call decoding, reply
encoding, fixed-page span checks, admitted body copy, exit decoding, and
verified scratch clearing. The composition harness reuses the accepted
scheduler frame, allocator-backed task spaces, supervision relationship, and
architecture-neutral rite admission instead of creating a second authority or
lifecycle model.

Generation 1 validates one exact compiled human source, derives its 17-byte
console body, completes console and empty-input rites, is preempted by the real
PIT, resumes, and enters the exact contained `#UD`. At the kernel-stack restart
checkpoint, its rite channel is retired before its task, circle, mappings,
pages, and privilege stack are destructively withdrawn.

Generation 2 proves zeroed reuse, receives fresh task/circle identities and a
fresh `shell_minimum` channel, and constructs the equivalent console request
directly. Its zero-status terminal exit completes the scheduled task, publishes
the supervision result, wakes the blocked supervisor, and makes the rite gate
restore the supervisor frame. No reply is copied and no `iretq` targets the
exited vessel.

## Observed acceptance evidence

**Observed:** `make shell-vessel-smoke` exited QEMU with debug status 33. The
strict verifier accepted exactly 38 schema-1 information records: eight boot
records and 30 composition records. It observed:

- six dispatches, one PIT preemption at tick 11, three resumes, two blocks, two
  wakes, two observations, one ELF-derived `#UD`, one restart, and one
  completion;
- five rites: generation-1 console/input sequences 1 and 2, then fresh
  generation-2 console/input/exit sequences 1, 2, and 3;
- two 17-byte console commits labeled `fixed_human_source` and
  `machine_direct`, with `semantic_equal=true`;
- terminal entry from user state, no reply copy, no return to the exited vessel,
  and exact supervisor selection;
- two retired channels and verified transport, scratch, user, stack,
  privilege-stack, transition-stack, and page-table clearing; and
- all active object, authority, supervision, scheduler, context, mapping, and
  rite-channel counts at zero, with all 65,102 initial free frames restored.

The verifier rejects lifecycle omissions, duplicates, reordering, malformed or
unknown records, field mutations, terminal-reply/return mutations, release
mutations, error/fatal events, unstructured output, private fields, source,
console output, traces, and fixture markers. Normal-image inspection excludes
the namespace, gate/runtime and fixture symbols, role/wait labels, complete and
split source/output probes, and all private stack and trace markers.

**Observed:** the complete acceptance regression set passed. Exact host counts
were eight ABI tests; kernel counts of 235 default, 240 circle, 235 control, 240
rite, 241 scheduler, 241 supervision, 246 shell-vessel, 235 memory, and 235
interrupt tests; and 35 xtask tests. Formatting, script syntax, host and
warning-denied x86-64 Clippy, every established bounded QEMU smoke proof, and
normal exclusion also passed.

**Reviewed:** an independent read-only audit found no actionable gate, unsafe,
lifecycle, privacy, or teardown defect. It confirmed one expected relocation
from the kernel gate entry to Rust and no relocations in any copied ring-3 blob.

## Claim boundary

This is not Canticle, a shell loop, a parser or machine AST, physical input,
general blocking, a production scheduler or supervisor, authenticated identity,
a service/package lifecycle, or a stable normal-image ABI. It is serialized on
one BSP with NMI masked, fixed non-hostile fixtures, deferred telemetry, and
fail-stop error paths. VirtualBox was not run for this acceptance.

The native kernel and rite boundary remain x86-64. IA-32 execution and 32-bit
Win32 ABI support remain isolated user-space compatibility-veil work that must
validate and translate foreign state into fixed-width native rites.

## Next question

The next foundation should use this composed mechanism without pretending the
fixed source projection is a language. The open work is to define the smallest
typed, privacy-preserving shell/session contract—plus nonempty input and
blocking semantics—that can be tested host-side before any general evaluator is
launched as a vessel.
