# ADR 0020: Compose a fixed supervised shell vessel

- Status: Accepted
- Date: 2026-09-13
- Accepted: 2026-09-13

This record accepts one deliberately fixed, feature-only composition proof.
It is not a user-space shell, Canticle implementation, stable syscall ABI,
service manager, package lifecycle, or production scheduling policy.

## Context

ADR 0015 proved bounded x86-64 scheduling and contained user failure. ADR 0018
proved that a persistent supervisor can observe one failed vessel generation,
authorize destructive replacement at a safe kernel-stack checkpoint, and
observe the replacement's clean completion. ADR 0019 separately proved a
fixed native rite gate for a single ring-3 shell-task fixture.

Before this decision, those mechanisms had not been exercised together. In
particular, the repository had not shown that a scheduled vessel could use the
rite boundary, survive preemption, be faulted without losing its supervisor,
receive a fresh task-bound rite channel after destructive restart, and
terminate through a rite without returning to dead user state.

Anomalous also requires human and machine clients to reach the same semantics
without maintaining separate authority models. The smallest honest experiment
is not a new shell language or machine protocol. It is two fixed projections
that converge on the same existing low-level console rite before any richer
syntax, AST, or exec-hag contract is chosen.

## Decision

This decision adds a removable `shell_vessel_test` feature and
`make shell-vessel-smoke` proof. It reuses the accepted architecture-neutral
scheduler, supervision, task, circle, and rite-admission mechanisms plus the
accepted 22-qword x86-64 saved-context shape. It does not change `anomalous-abi`.

The feature owns one persistent `supervisor_fixture` and two destructive
incarnations of one `shell_vessel_fixture`. Each incarnation receives exactly
the `shell_minimum` grant: `console_write | input_read | task_exit`. Its rite
channel is bound to its exact task incarnation and is retired before that
incarnation's storage or identity is withdrawn. A replacement receives a fresh
channel whose reply sequence starts at one.

The kernel remains natively x86-64. IA-32 execution and 32-bit Windows ABI
translation remain future user-space compatibility-veil work; they do not add
an IA-32 kernel target or a second kernel control surface.

### Coequal fixed projections

Generation 1 contains one compiled, fixed human-source fixture equivalent to
`echo Anomalous ready.` followed by a newline. Its ring-3 code validates only
that exact source form and projects it to an ordinary `console_write` call. It
then submits an empty `input_read`, which must return `would_block`. This is a
bounded projection test, not keyboard input, a general parser, or Canticle.

After destructive replacement, generation 2 constructs the same
`console_write` semantic tuple and bytes directly as a fixed machine fixture.
The kernel requires the operation, payload kind, declared right, bounds, body,
and result to equal the generation-1 projection while keeping their private
trace identities distinct. Serial evidence reports only the fixed projection
labels and `semantic_equal=true`; it never reports source, output, traces, or
mailbox contents. Generation 2 also observes the same empty-input result and
then requests `task_exit` with status zero.

This equality is deliberately below a future semantic command language. It
does not define a canonical AST, machine encoding, authenticated client
identity, policy envelope, or exec-hag API.

### Exact lifecycle

The proof must execute this bounded order:

1. Create the first shell incarnation and persistent supervisor in separate
   one-task circles, attach scheduler contexts and the supervision relationship,
   grant the shell a fresh rite channel, and dispatch it with IF clear.
2. Let generation 1 project the fixed human source, commit one private console
   payload through `screen_console`, receive `would_block` for empty input, and
   enable interrupts through the existing pointer-free scheduler gate.
3. Accept exactly one real PIT preemption, save generation 1, and dispatch the
   supervisor. The supervisor blocks awaiting the exact vessel fault, masks
   physical IRQs, and resumes generation 1.
4. Require generation 1 to execute the exact contained `ud2`. Publish one
   kernel-owned coarse fault observation, wake the supervisor atomically, and
   pivot the exception return to its preserved frame.
5. Let the supervisor consume that observation and park at the restart
   checkpoint. Return nonlocally to the original kernel stack, retire the
   generation-1 rite channel, then destructively clear and withdraw its task,
   circle, mappings, address space, user pages, and privilege stack.
6. Construct generation 2 with fresh task and circle generations, zeroed reused
   storage, and a fresh `shell_minimum` channel starting at sequence one. Admit
   and dispatch it through the same scheduler.
7. Let generation 2 construct the machine projection, prove semantic equality
   with generation 1 inside the kernel, commit it through the same rite path,
   observe empty input, and submit an exact zero-status `task_exit`.
8. Complete `task_exit` as one terminal composition: finish rite admission,
   mark the scheduled task exited, publish `successful_exit`, wake the exact
   blocked supervisor, construct but do not copy the success reply, consume the
   exact completion observation, and select the supervisor's validated frame.
   The exited vessel must receive no reply and must never be the target of
   `iretq`.
9. Let the resumed supervisor validate its wake and continuity, then complete
   through its single-use kernel continuation. Restore kernel processor state,
   retire the second channel, clear and withdraw both remaining tasks and
   circles,
   release every lease and relationship, verify initial accounting, emit the
   deferred structured transcript, and terminate the proof.

The transport mailbox is exclusively transport storage. Fixture continuity or
post-PIT markers belong in a fixed unused portion of the private user stack,
not at offsets that may alias a legal 4 KiB rite envelope. All user, mailbox,
scratch, page-table, and transition-stack storage is wiped before reuse or
release.

### Boundary and telemetry

The composition uses a feature-only DPL-3 x86-64 rite entry with the same fixed
40-byte call/reply prefixes and one-page mailbox limits accepted by ADR 0019.
Unlike the standalone proof entry, it returns a selected scheduler-frame
pointer so terminal exit can pivot to the supervisor. The ordinary image has no
such entry, fixture, namespace, source, output, or private marker.

The strict transcript is closed-world and contains only JSON records. It must
show the eight normal boot records followed by the exact composition lifecycle,
including six dispatches, one PIT preemption, one contained user fault, one
restart, two console commits, two empty-input results, one terminal exit, two
single-use observations, supervisor continuity, fresh rite sequence one after
replacement, full destructive withdrawal, and restored accounting. Console
source and output, trace values, mailbox bytes, pointers, roots, physical
addresses, registers, canaries, and private markers are forbidden from serial.

## Consequences

This proof establishes the first end-to-end skeleton of a restartable
ring-3 shell vessel: native rites, scheduling, containment, supervision, fresh
authority after replacement, human/machine semantic convergence, and terminal
exit all compose without moving policy-rich shell behavior into ring 0.

The cost is another sizable feature harness and a nonlocal terminal transition
that must be audited carefully. The fixed projections and display text are test
fixtures, not formats or APIs. The bootstrap ring-0 shell remains in place and
must continue to be described as bootstrap-only.

## Non-goals

This decision does not define or implement:

- a shell loop, editor, general parser, AST, Canticle runtime, pipeline, job
  control, command discovery, or executable loader;
- successful or blocking user-space input, keyboard command delivery, general
  wait queues, cancellation, timeouts, or signals;
- human/AI authentication, semantic authorization, package policy, sigil
  delegation, audit identity, exec-hag, or a machine command encoding;
- a stable rite, syscall, shell, scheduler, supervision, executable, package,
  or foreign-compatibility ABI;
- general user-copy, concurrent mailbox mutation, more than one relationship or
  restart, SMP, APIC, NMI enablement, DMA/IOMMU isolation, or production
  recovery policy; or
- a display manager, compositor, neumorphic interface, package manager, or
  migration of the bootstrap shell into the normal image.

## Acceptance evidence required

Before acceptance, the repository must provide:

- host tests for any extracted fixed mailbox transport and every new
  architecture-neutral composition invariant;
- a strict transcript verifier with exact key/value/order checks, ELF-derived
  fault location, privacy rejection, malformed/duplicate/unknown record tests,
  lifecycle and sequence mutations, terminal no-reply/no-return mutations, and
  release-accounting mutations;
- an x86-64 QEMU proof that exits successfully within the bounded timeout and
  produces the exact lifecycle with no unstructured output;
- normal-image inspection that excludes the feature namespace, linked fixture
  and gate symbols, human source, console output (including split instruction
  immediates), and every private marker; and
- all existing host, smoke, fault, circle, control, rite, memory, interrupt,
  shell, scheduler, supervision, and normal-exclusion regressions.

Acceptance must record exact observed counts and limitations. A passing fixed
fixture does not justify claims about a general shell language, production
service runtime, stable ABI, or physical input.

## Acceptance

Accepted on 2026-09-13 at implementation checkpoint `8da910a` after:

- `make check` passed eight `anomalous-abi` tests; kernel configurations
  with 235 default, 240 circle, 235 control, 240 rite, 241 scheduler, 241
  supervision, 246 shell-vessel, 235 memory, and 235 interrupt tests; 35
  `xtask` tests; formatting; script syntax; and warning-denied host and
  x86-64 target Clippy;
- `make smoke`, `make fault-smoke`, `make circle-smoke`,
  `make control-smoke`, `make rite-smoke`, `make scheduler-smoke`,
  `make supervision-smoke`, `make memory-smoke`,
  `make interrupt-smoke`, and `make shell-smoke` passed their established
  bounded QEMU checks;
- `make normal-exclusion` rejected the feature namespace, gate and runtime
  symbols, symbols for all three copied fixtures, role and wait labels,
  complete and split private source/output probes, and all private stack and
  trace markers in the ordinary image; and
- independent review found no actionable defect in the gate-frame construction,
  terminal transition, clearing order, feature precedence, or copied-blob
  relocation boundary.

`make shell-vessel-smoke` exited QEMU through debug status 33 and its strict
verifier accepted exactly 38 schema-1 information records: eight normal boot
records followed by 30 composition records. The observed lifecycle contained
six dispatches, one real PIT preemption at positive tick 11, three resumes, two
blocks, two wakes, two single-use observations, one ELF-derived contained
`#UD`, one destructive restart, and one clean completion.

Generation 1 issued `console_write` at sequence 1 and empty
`input_read` at sequence 2. Generation 2 received a fresh channel, restarted
at sequence 1 for its console rite, observed the same `would_block` input
result at sequence 2, and issued terminal `task_exit` at sequence 3. Both
console commits carried 17 private bytes through the same operation, payload,
rights, bounds, and result semantics. The verifier observed only
`fixed_human_source`, `machine_direct`, and `semantic_equal=true`; source,
output, trace identities, mailbox bytes, and private markers were absent from
serial.

Terminal exit entered from user state, finished the rite, marked the task
exited, published `successful_exit`, woke and selected the supervisor, copied
no reply, and never targeted the exited vessel with `iretq`. Teardown retired
both channels; wiped transport, scratch, user, stack, privilege-stack,
transition-stack, and page-table storage; restored kernel CR3, TSS RSP0, and
`CR0.TS`; returned every active object and scheduler count to zero; and
restored all 65,102 initially free frames.

This evidence remains one fixed, single-BSP, NMI-masked experiment with
fail-stop error handling and no hostile concurrent mailbox writer. It does not
implement Canticle, a shell loop, nonempty input, a production vessel/service
runtime, a stable ABI, authenticated human or machine identity, SMP, APIC,
DMA/IOMMU isolation, or a normal-image rite gate. VirtualBox was deliberately
not run for this acceptance.

The kernel and native rite boundary remain x86-64. IA-32 machine-code execution
and the 32-bit Win32 ABI remain versioned, isolated user-space
compatibility-veil work. Foreign pointers, handles, PE/NT structures, object
conventions, and calling conventions must be validated and copied before
translation into fixed-width native rites. This decision adds neither an IA-32
kernel target nor a second foreign kernel control surface.

## Alternatives considered

### Move the current bootstrap parser directly into ring 3

It currently depends on kernel-local console and command behavior rather than a
designed executable/runtime contract. Moving it now would disguise missing
input, loading, authority, and language decisions.

### Run the three accepted proofs sequentially

Separate scheduler, supervision, and rite images already pass. Sequential runs
cannot show that their task identity, context switching, restart, channel
lifetime, and terminal-return invariants compose.

### Define Canticle or exec-hag first

Those semantic surfaces need their own conformance and security work. A fixed
two-projection experiment can test the shared-mechanism premise without
prematurely stabilizing syntax or a machine protocol.

### Reuse the standalone rite return path unchanged

Its terminal exit returns directly to a saved kernel continuation. A supervised
scheduled vessel instead must atomically wake its supervisor and select that
task's saved frame; returning to the exited vessel or bypassing supervision
would fail the composition question.
