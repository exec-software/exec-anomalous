# ADR 0011: Prove the first circle with a fixed x86-64 prototype

- Status: Accepted
- Date: 2026-09-09

This record defines a deliberately fixed, test-only prototype circle. Its
addresses, gate vector, payloads, and lifecycle are experimental evidence, not
stable platform ABI.

## Context

The kernel now owns its GDT, TSS, exception IDT, ring-transition stack, and
fatal fault diagnostics. All normal code still executes at ring 0, and
Threshold's bootstrap mappings do not establish user memory isolation.

Entering ring 3 alone would prove only descriptor mechanics. The first useful
circle experiment must also prove that:

- only explicitly selected pages are available at CPL 3;
- executable and writable user mappings are distinct;
- guard pages remain absent;
- a present supervisor mapping cannot be read by user code;
- an NX user stack cannot be executed;
- each denial belongs to the exact offending instruction and lifecycle phase;
- private pages are cleared after a denied generation; and
- the kernel can start a clean replacement generation.

A general allocator, scheduler, loader, syscall ABI, IPC model, sigil system,
and production supervisor do not exist. Inventing them in this slice would hide
the hardware boundary under unrelated policy.

## Decision

### Scope

Add a `circle_test` build feature and bounded `make circle-smoke` image for the
bootstrap processor. The feature implies the existing smoke-test transport.
Normal images contain no user-callable test gate and do not enter ring 3.

The kernel constructs a separate page-table root rather than modifying
Threshold's tables in place. The first four GiB remain identity-mapped for the
kernel with effective supervisor-only access. A distinct lower-canonical PML4
branch contains the prototype's user mappings; no user bit is added to the
kernel identity-map hierarchy.

The test requires processor NX support. Before installing NX entries, the
kernel enables and reads back `EFER.NXE`; it also enables and reads back
`CR0.WP`. Failure to establish either mechanism stops the test before ring 3.

### Fixed virtual layout

Reserve one two-MiB window beginning at `0x0000_4000_0000_0000`:

| Virtual range | Purpose | Effective CPL-3 permissions |
| --- | --- | --- |
| `...0000..0fff` | Lower guard | Not present |
| `...1000..1fff` | Probe code | User, read, execute |
| `...2000..2fff` | Code/data guard | Not present |
| `...3000..3fff` | Generation scratch | User, read, write, NX |
| `...4000..4fff` | Data/stack guard | Not present |
| `...5000..5fff` | Down-growing stack | User, read, write, NX |
| `...6000..6fff` | Upper stack guard | Not present |
| Remaining window | Unassigned | Not present |

The initial stack pointer is the exclusive upper bound `...6000`. Code, data,
and stack use distinct page-aligned kernel-owned backing pages. Their identity
aliases remain supervisor-only. No user mapping exposes the kernel image, page
tables, descriptors, exception stacks, `boot_omen`, framebuffer, serial ports,
or arbitrary physical memory.

Code templates stay in the ordinary file-backed kernel image and are copied to
the user code page. They are never linked at the high user address, avoiding a
flat-image hole. Runtime and host checks validate effective permissions through
every paging level, not only leaf flags.

### Ring-3 entry

DPL-3 data and 64-bit code descriptors are appended after the two TSS slots so
existing kernel and TSS selectors remain unchanged. The transition uses
`iretq` with:

- the fixed entry and stack addresses;
- DPL-3 code, data, and stack selectors;
- `IF=0`, `IOPL=0`, `DF=0`, and otherwise sanitized flags;
- null FS and GS selectors; and
- cleared general-purpose registers except for fixed test values.

The TSS ring-transition stack receives every CPL-3 entry into the kernel. User
code receives no I/O permission.

### Test-only DPL-3 gate

Vector `0xf0` is present only under `circle_test`. Its `0xee` descriptor is a
DPL-3 interrupt gate targeting the kernel code selector. The gate accepts fixed
numeric observations only; it accepts no pointer, buffer, string, requested
operation, or authority.

The handler validates the exact lifecycle phase, report identifier, return
instruction address, DPL-3 selectors, and user stack bounds. It returns with
`iretq` only for an expected non-terminal observation. Any duplicate,
out-of-order, or malformed report is fatal.

This gate is a diagnostic rendezvous. It is not a syscall, rite, invocation,
IPC transport, or reserved future vector.

### Generation 1: supervisor-read denial

Generation 1 verifies writable scratch by observing zero, writing a fixed
residue, and reporting entry through vector `0xf0`. It then executes one
labelled read from a known, present, supervisor-only kernel canary.

Containment is allowed only when the frame matches every field:

- lifecycle phase `generation_1_denial`;
- vector 14;
- the exact faulting instruction pointer;
- DPL-3 code and stack selectors;
- a stack pointer within the mapped user stack;
- `CR2` equal to the canary address;
- error code exactly `0x5`—present protection violation, user read; and
- the expected scratch residue visible through the supervisor alias.

A not-present fault does not prove supervisor isolation and is rejected. The
canary value is never copied into user memory or telemetry. The faulting
generation is never resumed.

### Generation 2: NX-stack denial

After teardown, generation 2 must again observe zero scratch, write a distinct
residue, and report entry. It then jumps to a fixed instruction sequence placed
in its present user stack page.

The stack is writable but NX. Containment requires phase
`generation_2_denial`, exact stack target RIP and CR2, DPL-3 selectors and stack
bounds, error code exactly `0x15`—present user instruction fetch—and the second
scratch residue. If the stack instructions run and report, the test fails.

This is active NX evidence for the stack mapping. Data-page NX and code-page
read-only state are verified through table construction and walking in this
slice, not by separate behavioral faults.

### Teardown and clean restart

After either exact denial, the kernel:

1. marks the generation inactive and withdraws its user page-table root;
2. reloads the original CR3 before reusing private backing storage;
3. clears code, data, stack, and private paging pages with volatile writes;
4. verifies every byte is zero through supervisor aliases;
5. discards the saved user frame and resets the one-shot fault state; and
6. constructs a fresh generation from immutable templates.

Generation 3 is launched only after both denied generations were removed and
cleared. It must observe clean scratch, write a third residue, and make one
exact terminal ring-3 report. The kernel then performs a final teardown and
emits success. No instruction after either denied access may execute.

These steps demonstrate replacement of one fixed protection-domain instance.
They do not define general resource reclamation, identity, task restart policy,
or vessel supervision.

## Non-goals

This decision does not define or implement:

- general physical or virtual memory allocation;
- processes, threads, scheduling, timers, or preemption;
- multiprocessor execution or TLB shootdown;
- production system calls or exception recovery;
- IPC, rites, invocations, sigils, or policy;
- Reliquary or arbitrary executable loading;
- peer-circle isolation or a general supervisor;
- the Canticle runtime;
- external interrupt or NMI enablement; or
- a production user-space environment.

The prototype is not a vessel. That abstraction also requires general launch,
identity, resource accounting, supervision, and restart policy.

## Security limitations

The experiment proves narrow architectural facts on tested emulators: selected
CPL-3 accesses execute, the selected present supervisor read is denied, the
selected present NX fetch is denied, and each denied instance can be cleared
and replaced.

It does not prove resistance to speculative or other microarchitectural side
channels, safety on every processor, kernel page-table isolation, global W^X,
SMEP, SMAP, control-flow enforcement, kernel-stack guards, DMA isolation,
firmware isolation, confidentiality between principals, concurrent teardown,
or denial-of-service containment. Supervisor mappings remain present in the
active CR3, and existing kernel identity aliases remain writable and
executable. A looping user payload cannot be preempted because no timer exists.

Fixed addresses, static backing storage, disabled interrupts, unmasked layout,
and test-only recovery are explicit limitations. Fault addresses are acceptable
only on the current local bring-up channel and later require oracle disclosure
policy.

## Acceptance evidence

Host tests must cover descriptor and gate privilege bits, page-table indexes and
effective permissions, absent guards, exact fault predicates with one-field
mutations, report ordering, and volatile-clear verification.

`make circle-smoke` must parse complete structured records and require exactly
one occurrence of each event in order:

1. fault path and paging protections ready;
2. generation-1 mapping and DPL-3 entry;
3. exact supervisor-read denial and teardown;
4. generation-2 clean mapping and DPL-3 entry;
5. exact NX-stack denial and teardown;
6. generation-3 clean mapping and DPL-3 entry;
7. final teardown and `kernel.circle_test.passed`; and
8. QEMU debug-exit status 33.

The harness rejects unexpected, nested, fatal, duplicate, missing, or reordered
events; any after-denial marker; early exit; reset; signal; or timeout. Normal
image symbol and string inspection plus the ordinary boot and shell tests must
show that the test gate, payloads, and terminal events are absent.

`make check`, `make smoke`, `make fault-smoke`, and `make shell-smoke` remain
regression gates.

## Consequences

The project gains direct evidence for its first memory-isolation boundary and a
narrow fault-and-replacement sequence. It also gains architecture-specific page
tables, transition assembly, and a carefully constrained recoverable test path,
which increase privileged code and require exact invariants.

The layout, static pages, gate vector, numeric reports, and payloads may be
removed without compatibility support. A later ADR must define general circles,
address-space ownership, tasks, scheduling, and production entry/exit semantics.

## Acceptance evidence recorded

The decision was accepted on 2026-09-09 after the implementation checkpoint
`1d11e3b` and its follow-up review established the required evidence:

- host checks cover DPL-3 descriptors and gate attributes, fixed page-table
  indexes, effective permissions and absent guards, exact fault matching with
  one-field mutations, NX error semantics, and observable volatile clearing;
- `make circle-smoke` derives the copied generation-1 fault RIP and canary CR2
  from the built ELF, requires every structured field and lifecycle event in
  order, rejects false-positive paths, and observes QEMU debug-exit status 33;
- QEMU produces exact page-fault errors `0x5` and `0x15`, then a clean terminal
  third generation, with all three teardowns verified;
- the normal-image scan finds none of the test gate, payload symbols, event
  namespace, or generation residue markers;
- the ordinary boot, diagnostic-fault, and bootstrap-shell tests remain green;
  and
- the dedicated VirtualBox VM produces the same complete lifecycle before the
  normal interactive image is restored.

VirtualBox sets the architectural resume flag in saved page-fault RFLAGS while
QEMU leaves it clear. Containment therefore accepts only sanitized flags with
that single optional bit (`0x2` or `0x10002`); every field named by the exact
fault contract remains identical. The recovery trampoline also restores the
incoming CR0 task-switched state.

## Alternatives considered

### Treat ring-3 entry as sufficient

This proves descriptor mechanics but not useful page permissions, denial
attribution, clearing, or replacement.

### Read an unmapped address

A not-present fault cannot distinguish deliberate isolation from an incomplete
table. The probe uses a present supervisor mapping and requires error `0x5`.

### Resume after the denied instruction

Advancing RIP would demonstrate exception continuation, not containment. The
faulted frame is discarded and a fresh generation is constructed.

### Define a general syscall ABI now

That would force premature decisions about tasks, copying, IPC, authority,
cancellation, and concurrency. The fixed diagnostic gate remains feature-local.

### Build the allocator and scheduler first

They are required for general circles but add enough state to obscure the basic
paging and transition proof.
