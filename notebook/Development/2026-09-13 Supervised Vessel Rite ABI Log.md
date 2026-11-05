---
title: "Supervised Vessel Rite ABI Log"
date: 2026-09-13
tags: [kernel, abi, rites, shell, display, vessels, supervision]
status: accepted
domain: operating-system
---

# Supervised Vessel Rite ABI Log

## Goal

Create the first narrow kernel/user boundary that can support a supervised shell
vessel and a supervised display-manager vessel without turning either into a
ring-0 subsystem.

## Initial slice

Added allocation-free `rite_call` and `rite_reply` prefixes to
`anomalous-abi`, with explicit operation IDs, payload classifications, status
values, rights bits, and host tests for failure-closed prefix validation.

The minimum grants are intentionally separate:

- shell vessel: `console_write`, `input_read`, `task_exit`;
- display manager vessel: `display_update`, `input_read`, `task_exit`.

This is not yet a syscall gate. It is the shared wire/layout contract that a
feature-only x86-64 proof can consume next.

## Kernel admission slice

Added an architecture-neutral kernel rite module around the ABI prefix. One
non-copyable channel binds a trusted rights grant and reply sequence to an exact
generational task identity. Caller-declared rights may narrow that grant but
cannot add authority, and a stale vessel incarnation is denied.

The module admits no payload pointer or bytes. It produces a non-copyable,
sanitized guard for later operation-specific preflight, or a fixed payload-free
rejection. Admission and completion query the real task table for the exact
running incarnation. The guard exclusively borrows its channel, while a
kernel-owned pending marker survives even deliberate forget; dropping,
forgetting, or finding the task stale makes continuation terminal. This prevents
a later call from overtaking a pending operation.

Prefix admission enforces checked envelope length, a kernel-owned 4 KiB maximum,
exact zero-length input and yield requests, an exact eight-byte task-exit size,
and typed console/display metadata shapes. It does not validate UTF-8, task-exit
reserved bytes, or display-patch contents because no payload bytes enter this
layer. Structured console events remain rejected until their schema exists.

All attempts, including rejections, consume a monotonic sequence. The channel
issues u64::MAX once and then reports terminal internal exhaustion instead of
wrapping.

## Native architecture checkpoint

The end target is x86-64, with a native x86-64 kernel and native fixed-width
rite transport. Required IA-32 machine-code execution and 32-bit Win32 ABI
compatibility are a future user-space compatibility-veil concern: Win32/NT
pointer widths, handles, PE loading, and
calling conventions must terminate at that translator and become ordinary
native rites. No IA-32 kernel target is implied.

An ABI audit found that the original rite_reply relied on four bytes of
implicit x86-64 alignment padding before sequence. That is unsuitable for a
kernel/user copy boundary and would also make the layout target-dependent.
The prefix now names those bytes as an explicit zero-required u32, and
compile-time size/offset assertions pin the call, reply, and exit layouts.

The x86-64 feature gate must remain client-neutral: it recovers an exact task
incarnation and trusted grant from kernel state, regardless of whether the
eventual caller projection is a person-facing shell, an AI client, or exec-hag.

## Implemented x86-64 gate slice

The planned slice became a removable `rite_test` feature and
`make rite-smoke` target. The kernel and gate remain natively x86-64. Future
IA-32 machine-code and 32-bit Win32 ABI compatibility must terminate in a
user-space veil and translate into the same fixed-width native rites; this work
does not create an IA-32 kernel target.

The fixed ring-3 shell-task fixture models a future supervised vessel but is not
attached to a supervision relationship. It receives only RX code, one RW+NX
mailbox, and an RW+NX stack with absent guards. DPL-3 vector `0xf3` accepts no
caller-selected task or payload pointer. The kernel resolves the exact running
task and trusted `shell_minimum` grant, so the transport does not distinguish a
future human shell from an AI client or exec-hag.

The mailbox is one 4 KiB page. Its 40-byte call prefix leaves a deliberate
4,056-byte test-transport body cap beneath the admission layer's independent
4 KiB policy. The implemented ordering is:

1. copy the fixed call prefix exactly once through the supervisor physical
   alias;
2. reject bad header, unknown operation, malformed metadata, untrusted rights,
   stale task state, or invalid length before reading body bytes;
3. copy only an admitted body once into zero-initialized fixed kernel storage;
4. finish fallible content validation before the unit-returning commit; and
5. serialize one padding-free reply with every reserved byte explicitly zero.

The exact nine-call ring-3 sequence is:

1. unknown operation to `bad_operation`;
2. missing console right to `denied`;
3. display-right overclaim to `denied`;
4. 4,057-byte body above the transport cap to `limit_exceeded`;
5. copied invalid UTF-8 to `bad_payload`;
6. 16 private printable bytes committed through `screen_console` to `ok`;
7. empty `input_read` to no-commit `would_block`;
8. copied nonzero exit reservation to `bad_payload`; and
9. exact `task_exit` to terminal `ok`.

The first eight replies return through `iretq` and are checked completely by the
ring-3 fixture. Successful `task_exit` copies no reply and instead consumes a
single-use continuation to reach the saved kernel stack without resuming the
exited task. Teardown then restores CR3 and `CR0.TS`, unpublishes the gate
context, wipes the TSS stack and continuation cells, withdraws and clears the
three user mappings and private page tables, destroys task/circle state, releases
leases, and verifies initial accounting.

## Observed evidence

Implementation checkpoint `7d3a872` completed the fixed proof, which was
accepted after:

- `make check` passed eight `anomalous-abi` tests and eight kernel configurations
  (the rite-feature configuration ran 235 tests), plus 31 xtask tests,
  formatting, script syntax, and warning-denied host and x86-64 target Clippy;
- `make rite-smoke` accepted exactly 20 structured records: seven boot records,
  ready and task-entry records, nine ordered outcomes, release, and pass;
- the rite transcript reported calls 9, rejections 6, commits 2,
  `would_block` 1, terminal true, and no private sentinel, unstructured output,
  error, or fatal event;
- release restored all 65,116 initial free frames, zero data-owned, table-owned,
  active-circle, active-task, and active-mapping counts, retired the channel, and
  reported the mailbox and transition stack wiped;
- runtime checks verified exact gate frames, replies, copy ordering, terminal
  no-reply/no-`iretq` behavior, kernel CR3 and `CR0.TS` restoration, and cleared
  user/page-table/private scratch storage;
- host tests covered ABI layout and reserved bytes, grant separation, admission
  ordering, selected transcript mutations, privacy rejection, and normal-image
  exclusion; and
- `make smoke`, `make fault-smoke`, `make circle-smoke`, `make control-smoke`,
  `make memory-smoke`, `make interrupt-smoke`, `make shell-smoke`,
  `make scheduler-smoke`, `make supervision-smoke`, and
  `make normal-exclusion` all passed.

ADR 0019 is accepted for this bounded evidence. The host suite does not claim
that every gate-frame field was mutation-tested: complete frame and returning
reply validation occurs in the QEMU runtime, while host mutations exercise the
strict transcript and exclusion validators.

## Claim boundary

This is one serialized BSP-only shell-task fixture with IF clear and NMI masked.
It does not prove concurrent mailbox mutation, preemption, SMP, general
user-copy, nonempty input delivery, general blocking, composition with ADR
0018's supervision relationship, a user-space shell or display manager,
authenticated human/AI identity, a production vessel runtime, or a normal-image
or stable syscall ABI. The private console body is rendered by the feature
console commit but never appears in COM1 or a structured event.

## Design notes

Unknown operation numbers are data and fail as `bad_operation`, not undefined
behavior. Unknown payload kinds or rights bits fail as `bad_payload`. Missing
rights fail as `denied`. Claimed payload length greater than the caller-supplied
limit fails as `limit_exceeded` before any payload interpretation.

Kernel admission freezes the next validation order: exact header, known
operation, known structural metadata, declared operation right, trusted
incarnation and grant, operation/payload-prefix compatibility, then checked
bounds. The caller's `payload_limit` is a declaration rather than a security
limit; per-operation kernel caps remain authoritative. The architecture gate
safely copies once into fixed kernel storage and validates copied content. The
admission guard then rechecks live task state. A successful
non-input rite can enter only a unit-returning, infallible bounded commit
callback. Its reply is constructed only after that callback returns. Input
would-block and all error outcomes use separate no-commit completion paths, so
no callback may
produce an effect and then downgrade the result. The ABI helper's declaration
checks and the kernel's authoritative admission policy are intentionally
layered; an ABI-level success is never execution authority.
