# ADR 0019: Define the first supervised-vessel rite ABI

- Status: Accepted
- Date: 2026-09-13
- Accepted: 2026-09-13

This record accepts the first deliberately small kernel/user boundary shape for
future shell and display-manager vessels. It is not a stable syscall ABI, not a
Canticle runtime, not a package lifecycle, and not a promise that the current
operation numbers survive pre-alpha.

## Context

ADR 0018 proved that a persistent supervisor can observe one failed vessel
generation, authorize a bounded destructive restart, and resume after a clean
replacement. The next foundational question is what a useful vessel can ask the
kernel to do without becoming another privileged ring-0 subsystem.

The current bootstrap shell is intentionally in-kernel. It is useful for bringup
and tests, but it is not the final shell. The display manager should likewise
become a supervised user-space vessel rather than a graphics policy engine
inside the kernel. Both need a shared, typed, capability-explicit boundary that
works for people and machines.

The native kernel and rite transport target is x86-64. Required IA-32
machine-code execution and 32-bit Win32 ABI compatibility belong in a
versioned user-space compatibility veil that translates foreign pointer widths,
calling conventions, handles, and object models into the same fixed-width
native rites. It does not create an IA-32 kernel target or let a foreign ABI
become the kernel's control surface.

## Decision

Add a transport-free `rite` prefix to `anomalous-abi`. A rite is the smallest
low-level kernel operation a launched vessel may request after it already holds
the relevant authority. This ABI layer is below Canticle and below the semantic
control envelope: Canticle may compile to rites, and machines may construct the
same structured requests directly, but neither route receives ambient authority.

The first operations in this pre-alpha shape are:

- `console_write`, for bounded diagnostic or terminal text/event output;
- `input_read`, for one bounded physical-input delivery;
- `task_exit`, for explicit vessel termination;
- `task_yield`, for cooperative scheduling without authority transfer; and
- `display_update`, for bounded framebuffer/display-surface updates.

Each operation maps to one required right bit. Unknown operations remain ordinary
integer data and fail closed as `bad_operation`. Unknown payload kinds, reserved
fields, unknown rights bits, and oversized payload claims fail before payload
interpretation. Missing rights fail as `denied`. Empty input queues should return
`would_block` rather than spinning or implicitly blocking.

The `rights_present` field is a caller declaration, not authority. It may narrow
the trusted grant recovered by the kernel for one request, but every declared
bit must be a subset of that grant and the operation's required bit must appear
in both. Grants and reply sequences bind to an exact task incarnation; a
replacement vessel receives a fresh grant and sequence.

Define two initial grants without merging policy:

- `shell_minimum = console_write | input_read | task_exit`; and
- `display_manager_minimum = display_update | input_read | task_exit`.

The split is intentional. A text shell should not automatically submit display
patches, and a display manager should not automatically write terminal/diagnostic
text. Future sigils may grant both, but the minimum vessels stay distinct.

## Consequences

This is the first reusable shape for moving the bootstrap shell and display
manager out of ring 0. It gives the kernel a small, auditable contract to build
on before richer Canticle, package, compositor, or foreign-compatibility policy
enters the picture.

The cost is committing to another pre-alpha ABI surface that must be guarded
carefully. The ABI crate is consumed by an architecture-neutral kernel admission
module and a removable x86-64 proof transport. The normal image still has no
rite gate, loader, general syscall table, compositor, package resolver,
compatibility veil, or userland shell through this decision.

The admission module holds a kernel-owned grant and monotonic, non-copyable
sequence for one exact task incarnation. It enforces an independent 4 KiB cap,
checked envelope length, and operation/payload-prefix compatibility. It returns
either a payload-free rejection or a non-copyable prepared guard containing only
sanitized metadata. Admission and completion both query the kernel task table
for the exact running incarnation. The guard exclusively borrows the channel
and persists a pending marker. Fallible body validation and execution preflight
must finish before completion. Successful non-input operations then expose only
an infallible, unit-returning commit path that constructs the reply afterward;
input would-block and all errors use no-commit paths. An abandoned, forgotten,
stale, or miscompleted guard closes the channel. The module never receives a
payload pointer or payload bytes, so content validation remains
operation-specific.

## Acceptance evidence required

Before acceptance, the repository needs a kernel proof that a fixed ring-3
shell-task fixture modeling a future vessel can invoke at least `console_write`,
`input_read` with `would_block`, and `task_exit`
through a fixed feature-only x86-64 gate. The proof must show exact rights
checks, size limits, unknown operation rejection, no private payload leakage in
serial telemetry, and normal-image exclusion of all feature-only gate and
fixture payloads. Every copied prefix must have a padding-free, compile-time
checked layout; all reserved reply bytes must be written as zero.

Host tests must also prove the allocation-free ABI layout, known/unknown value
behavior, shell/display grant separation, and failure ordering: operation and
header validation before payload interpretation, rights before execution, and
size checks before copying.

The ABI helper validates only the self-consistency of a caller declaration. The
kernel admission layer owns authoritative grant, liveness, metadata-shape, and
size policy, including its documented error precedence; success from the ABI
helper alone never permits execution.

## Acceptance

Accepted on 2026-09-13 at implementation checkpoint `7d3a872` after:

- `make check` passed eight ABI tests and eight kernel configurations (the
  rite-feature configuration ran 235 tests), plus 31 xtask tests,
  warning-denied host and x86-64 target Clippy, formatting, and script syntax;
- `make smoke`, `make fault-smoke`, `make circle-smoke`, `make control-smoke`,
  `make memory-smoke`, `make interrupt-smoke`, `make shell-smoke`,
  `make scheduler-smoke`, and `make supervision-smoke` all passed;
- `make rite-smoke` passed its QEMU proof; and
- `make normal-exclusion` rejected the rite feature namespace, linked helpers,
  fixture symbols, and private marker in the ordinary image.

The removable x86-64 image installs DPL-3 vector `0xf3` for one fixed ring-3
shell-task fixture with the `shell_minimum` grant. Its one-page mailbox holds a
40-byte call prefix and at most 4,056 body bytes. The exact nine-call sequence
produced six payload-free rejections and three non-error outcomes:

1. sequence 1 rejected an unknown operation as `bad_operation`;
2. sequence 2 rejected a missing console right as `denied`;
3. sequence 3 rejected a display-right overclaim as `denied`;
4. sequence 4 rejected a 4,057-byte transport body as `limit_exceeded`;
5. sequence 5 copied and rejected invalid UTF-8 as `bad_payload`;
6. sequence 6 committed 16 private printable bytes through `screen_console` and
   returned `ok`;
7. sequence 7 returned `would_block` for empty input without a commit;
8. sequence 8 copied and rejected a nonzero exit reservation as `bad_payload`;
   and
9. sequence 9 committed `task_exit`, copied no reply, and returned directly to
   the saved kernel continuation without `iretq`.

The strict verifier accepted exactly 20 structured records. It exposed no
private console body, trace identity, arbitrary mailbox content, address, or
register. Final evidence restored all 65,116 initial free frames, zeroed active
circle, task, mapping, data-owned, and table-owned counts, retired the channel,
and verified mailbox, user/page-table storage, transition stack, CR3, and
`CR0.TS` restoration.

Runtime validation checks the complete 22-qword gate frame and every returning
reply. Host tests prove the ABI layout, known/unknown values, shell/display grant
split, admission ordering, selected transcript mutations, privacy rejection,
and normal-image exclusion. This is not a claim of host-side mutation coverage
for every gate-frame field.

The accepted proof remains one serialized, BSP-only shell-task fixture with IF
clear and NMI masked. It is not attached to the ADR 0018 supervision relationship
and does not prove concurrent mailbox mutation, preemption, SMP, general
user-copy, nonempty input delivery, blocking, a user-space shell, authenticated
clients, a production vessel runtime, or a normal-image/stable syscall ABI.

## Alternatives considered

### Reuse the semantic invocation envelope directly

The semantic envelope is for higher-level intent, preview, and commit flows. The
kernel still needs a narrower internal boundary for simple operations after a
vessel has already been admitted with explicit rights.

### Make IA-32 or a foreign application ABI native to the kernel

That would couple the substrate to legacy pointer widths, calling conventions,
and object models. A compatibility veil can preserve Windows and other 32-bit
application behavior while translating into one x86-64-native, client-neutral
rite boundary.

### Add a POSIX-like syscall table now

That would import process/file assumptions before Anomalous has packages,
objects, display surfaces, or Canticle effects. The rite prefix keeps the first
contract smaller and typed.

### Keep shell/display in ring 0 longer

That would postpone the supervision lesson we just proved. The design goal is to
make policy-rich services restartable vessels while the kernel provides the
small hard substrate.
