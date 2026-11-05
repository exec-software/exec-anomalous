# ADR 0013: Prove bounded interrupt, time, and input delivery

- Status: Accepted
- Date: 2026-09-09

This record accepts a deliberately bounded bootstrap-processor experiment. Its
legacy controller programming, vectors, queue capacities, event records, and
test hooks would remain replaceable pre-alpha mechanisms, not stable platform
ABI.

## Context

Before this decision, the kernel owned its exception IDT, dedicated fatal
stacks, paging protections, and bounded bootstrap memory manager, but maskable
external interrupts remained disabled. NMI remained masked, no clock source was
established, and the temporary bootstrap shell polled the PS/2 controller for
input. That prevented preemption, lost input while the kernel was not polling,
and provided no trustworthy way to order externally triggered work in time.

An interrupt path is not just another exception path. Current exception entry is
deliberately non-returning except for two exact feature-test recoveries. A device
interrupt must instead attribute its source, bound its work, acknowledge the
controller exactly once, and return to the interrupted context. Sharing implicit
recovery rules between these paths would make both harder to audit.

The current `boot_omen` contains no ACPI tables, MADT topology, or other
discoverable interrupt-controller description. Selecting local APIC, I/O APIC,
or multiprocessor routing now would therefore depend on undeclared machine
assumptions. The existing QEMU and VirtualBox development machines do expose
legacy PC-compatible PIC, PIT, and 8042 devices, which are sufficient for a
bounded bootstrap proof if their use remains isolated behind a replaceable
backend.

Input also crosses an authority boundary. Physical keyboard transitions are
private device observations, not semantic requests and not proof of a person or
AI principal's authority. An AI client must eventually act through authenticated
IPC and versioned invocations, with attributable identity and policy, rather
than by inserting bytes into the keyboard stream. A debug serial channel must
not quietly become a second control surface.

## Decision

### Scope and platform boundary

Implement a bounded, BSP-only x86-64 interrupt, time, and keyboard-input
proof. A replaceable `legacy_pc` platform backend owns the dual 8259 PIC, PIT
channel 0, and 8042 keyboard interface on the current emulator profile. The
backend exposes typed kernel mechanisms rather than controller ports or scan
bytes to its consumers. Its documented boundary must remain open to independent
backends without granting any backend a private semantic-control interface.

The kernel masks every PIC line before reconfiguration. It installs all entry
state, initializes fixed source storage, drains or accounts for stale 8042
output, and programs a bounded periodic PIT timebase before executing `sti`.
Only the timer and a successfully initialized keyboard line are unmasked; no
other IRQ line is enabled. Failure of shared gate, PIC, PIT, or source
prerequisites leaves maskable interrupts disabled. A bounded 8042-specific
failure is an explicit degraded state: IRQ1 remains masked and unavailable
while the independently ready timer may run.

This backend exists because the handoff does not yet carry ACPI/MADT discovery;
it is not a claim that legacy PIC/PIT/8042 hardware is the Anomalous platform
model. A later ACPI-aware APIC backend must be able to replace it without
changing the source-record, timebase, input-privacy, or consumer contracts. The
kernel must not infer interrupt topology from emulator-specific defaults or ask
Threshold to invent topology it did not discover.

NMI remains masked throughout this slice. Multiprocessor startup and interrupt
routing are outside the proof.

### Returning IRQ entry

External IRQ vectors use dedicated assembly stubs and a returning Rust
dispatcher, distinct from the non-returning exception dispatcher and from any
test-only DPL-3 gate. Entry clears the direction flag, saves every
general-purpose register, normalizes a fixed frame without confusing a hardware
IRQ for a processor error-code exception, and aligns the stack for the Rust ABI.
An interrupt gate keeps maskable interrupts disabled while a handler is active;
the first proof does not permit nested external IRQ handling.

Initialization proves that every returning vector names its intended live stub
before delivery can be enabled. The dispatcher then validates the entry frame,
vector range, live arming state, and attribution to the configured source. It
performs bounded, allocation-free source work, issues the required PIC
end-of-interrupt exactly once and in controller order, restores the saved state,
and returns with `iretq`. Spurious PIC behavior must be detected and acknowledged
according to the controller contract without publishing a false device
observation.

An impossible entry frame, unexpected armed-source mismatch, acknowledgement
invariant failure, or exhausted nonwrapping identity enters the common early
fatal disposition: execute `cli`, emit only static or privacy-safe diagnostics
to COM1 and the available human boot surface, then remain in an `hlt` loop. A
fatal interrupt record starts at a fresh COM1 record boundary so preemption
cannot splice it into a foreground structured record. A fatal interrupt path
never returns.

### Fixed typed source records

Each enabled record-producing source owns fixed-capacity, allocation-free
storage and publishes only its own record type. The periodic timer advances the
private time service described below rather than producing queued records.
There is no untyped global byte stream and no caller-provided pointer in IRQ
context. Every published record carries:

- a nonzero, nonwrapping `source_generation` assigned when that source is
  initialized or explicitly reset;
- a source-local, nonwrapping `sequence` that advances for every completed
  physical observation or explicit source discontinuity, including a completed
  observation lost because its queue was full; valid scan-code framing bytes do
  not invent physical-event identities; and
- the current monotonic timebase identity and tick observation.

The timer source advances a private monotonic observation with a nonwrapping
`timebase_generation` and nonwrapping tick count. This slice creates immutable
boot timebase generation 1 and does not reprogram or rotate it. Ticks are
monotonic and directly comparable only within the same timebase identity. A
future reset or replacement must allocate a fresh nonwrapping generation and
must not order different generations by their numeric ticks. Wall-clock meaning
is not inferred from PIT ticks. Raw periodic ticks are not broadcast into a
global event queue; future timers will expose owner-bound deadlines instead of
making every consumer process clock traffic.

Each source has a fixed loss latch outside its ordinary queue. If the queue is
full, the producer records the exact first and last missing sequence and a
checked loss count rather than overwriting an older record or silently dropping
the newest one. Before later ordinary records from that source are exposed, the
consumer receives a typed loss record. A keyboard loss invalidates any derived
pressed-key state until the input consumer explicitly resynchronizes. A counter
or generation that would wrap exhausts the source and takes the fatal path;
identity is never silently reused.

The IRQ producer never allocates, blocks, formats text, invokes shell behavior,
or calls a machine client. Consumer work is bounded per drain so one noisy
source cannot monopolize the bootstrap processor.

### Physical input and layout separation

The 8042/input layer decodes bytes into typed physical key transitions: physical
key identity, press or release state, source generation, sequence, and timebase
observation. Its scan-set-1 decoder owns prefix, make/break, and malformed-stream
framing. The surrounding input transport owns explicit loss and the controlled
reset boundary; a bounded architecture-owned 8042/device reinitialization must
complete before the decoder and source generation can restart, and successful
initialization or rearm resets the empty-IRQ storm streak before IRQ1 is
unmasked. Steady-state transport, response, and device-error bytes `0x00`,
`0xee`, `0xfa`, `0xfc`, `0xfd`, `0xfe`, and `0xff` cannot become physical key
transitions; they invalidate partial framing and require explicit loss and
rearm. An unsolicited steady-state `0xaa` byte remains the valid left-Shift
release it is indistinguishable from, not an inferred device-reset signal.
Text, keyboard layout, modifiers with textual meaning, Unicode, composition,
shortcuts, shell editing, and application actions remain outside this layer.

The temporary bootstrap shell may use a separate, explicitly provisional US
ASCII layout adapter to preserve the current bring-up workflow. That adapter
consumes physical transitions outside IRQ context. Its mapping is not a kernel
input ABI, Canticle syntax, or evidence that the eventual input familiar has
been designed.

Raw controller bytes and physical key records are private. They must not be
written to COM1, copied into general boot telemetry, or exposed to an AI client.
Privacy-safe diagnostics may report static source identity, lifecycle state,
bounded counts, and explicit loss without reporting scan bytes, physical keys,
derived text, or shell content.

### Human and AI authority separation

This proof treats a person's physical input as one mechanism for reaching a
future semantic control plane, not as the control plane itself. It grants no
authority merely because a transition arrived from the keyboard controller.
Authorization remains a later policy decision made against the target operation
and active principal.

AI clients will use future authenticated IPC and versioned invocations. They
must not impersonate physical input, enqueue fabricated keyboard transitions,
or depend on scan-code timing to control the system. Human UI, shell, and AI
clients can eventually project the same operations and policy while retaining
distinct, attributable origins. Accessibility or remote-input mechanisms, when
designed, must likewise use explicitly typed and authorized origins rather than
claiming to be physical 8042 input.

COM1 remains an output-only early diagnostic transport. The normal kernel does
not interpret serial bytes as commands, key events, invocations, test control,
or authorization. Feature tests may use emulator-owned injection facilities,
but those hooks and records must be absent from the normal image.

### Observable lifecycle

Normal boot may emit a privacy-safe readiness record after controller masks,
source generations, queue state, timebase configuration, and returning entry
paths are established. It must not claim readiness merely because the PIC was
programmed. Human and machine diagnostics expose the same readiness, loss, and
fatal truth without exposing raw input.

A feature-only interrupt test exercises real PIT interrupts and injected 8042
make and break traffic. Its strict transcript must demonstrate:

1. exceptions remain installed on their non-returning diagnostic path;
2. readiness reports the initial all-masked state and final masks, while host
   port-write tests prove only configured timer and healthy keyboard lines
   become armed;
3. PIT ticks advance under one identified monotonic timebase;
4. exact physical press and release transitions arrive in source order with
   attributable generations, sequences, and tick observations;
5. execution returns from both timer and keyboard IRQ contexts;
6. deterministic test-only queue pressure produces an exact loss range before
   later records, contains IRQ1, and requires a bounded 8042/device
   reinitialization before a fresh source generation restarts at sequence zero;
7. no raw scan byte, physical key, or derived private text reaches COM1; and
8. the feature reports success only after interrupt masking is restored for
   bounded test exit.

For this fixed proof, readiness reports initial masks `0xff`/`0xff` and armed
masks `0xfc`/`0xff`. The first 24 transitions occupy generation 1 sequences 0
through 23; deterministic pressure then queues sequences 24 through 87 and
records one exact capacity loss at sequence 88. The lifecycle must show IRQ1
contained at master mask `0xfe`, perform successful bounded 8042/device
reinitialization, and only then report source generation 1 advancing to 2 with
master mask `0xfc` after the fresh source is installed. Injected post-rearm input
must arrive as generation 2 sequences 0 through 1 before final masks return to
`0xff`/`0xff`.

The ordinary shell smoke test must demonstrate that the separate layout adapter
still supports editing and dispatch without leaking input. Normal-image
inspection rejects the named interrupt-test entry and forced-loss helpers, the
`kernel.interrupt_test.` event namespace, and the injected privacy probe used by
this feature proof.

## Required invariants

- No maskable interrupt is enabled before its gate, source state, queue, handler,
  acknowledgement route, and fatal fallback are complete.
- Exception entry remains non-returning by default; external IRQ entry returns
  only after bounded handling and correct acknowledgement.
- Only the configured timer and keyboard sources are unmasked in this proof.
- A source generation, sequence, timebase generation, or tick is never reused
  after wrap.
- Per-source FIFO order is stable, and every missing range is explicit before a
  later ordinary record from that source is delivered.
- A lost keyboard transition invalidates derived key state until explicit
  resynchronization.
- PIT ticks provide monotonic ordering within an attributable generation, not
  wall-clock time.
- IRQ context performs no allocation, unbounded loop, text formatting, semantic
  command dispatch, IPC, or user-pointer access.
- Layout and text interpretation cannot mutate the private physical-transition
  record or masquerade as its origin.
- AI activity cannot enter through the physical keyboard source; future machine
  control requires authenticated IPC and invocation provenance.
- COM1 is diagnostic output only and never contains raw or derived private
  input.
- Fatal interrupt state disables maskable interrupts and halts instead of
  returning into uncertain execution.

## Non-goals

This decision does not define or implement:

- ACPI discovery, MADT parsing, local APIC, I/O APIC, x2APIC, MSI, or MSI-X;
- multiprocessor startup, interprocessor interrupts, routing, or balancing;
- NMI enablement or recoverable exception policy;
- HPET, invariant-TSC calibration, RTC, wall-clock time, deadlines, or timers;
- a scheduler, preemption policy, threads, task accounting, or general waits;
- a stable interrupt, input, time, syscall, or device-driver ABI;
- a user-space input familiar, general IPC, or device delegation;
- keyboard layouts beyond the provisional shell adapter, Unicode, composition,
  accessibility input, pointer devices, touch, audio, or remote input;
- authentication of a person from keyboard presence or secure-attention policy;
- AI IPC, exec-hag, rites, invocations, sigils, Canticle, or shell authority;
- serial-console input, remote command execution, or a debug backdoor; or
- production hardware breadth, performance, power management, or recovery.

## Security limitations

Legacy PIC, PIT, and 8042 devices are trusted emulator mechanisms in this
bounded proof. They provide no cryptographic source identity, user identity, or
protection against malicious firmware, emulated devices, electrical injection,
timing leakage, interrupt storms, or compromised ring-0 code. Interrupts still
execute on one bootstrap processor with the current identity map and fixed
kernel stacks.

Fixed queues bound memory use but permit denial of service and explicit event
loss. Keeping raw input out of serial telemetry reduces disclosure; it does not
yet provide an isolated input path, trusted user attention, encrypted storage,
or a complete oracle retention policy.

## Acceptance evidence

Host tests must cover PIC initialization, masks, spurious paths, EOI order, the
exact PIT port sequence (`0x43 <- 0x34`, `0x40 <- 0x9c`, `0x40 <- 0x2e`) and
bounds, 8042 prefix/make/break/error transitions, empty-IRQ streak reset,
separation of physical identity from layout, queue FIFO and wraparound indexes,
deterministic full-queue loss, loss-before-later-record ordering, key-state
invalidation, source-generation, sequence, tick, and loss-count exhaustion, and
privacy-safe event formatting.
Entry-frame tests and image inspection must distinguish returning IRQ stubs from
exception and test-gate paths.

A bounded QEMU gate must observe real periodic IRQ delivery, injected keyboard
press and release traffic, continued execution after both handlers, exact typed
records, deliberate feature-only loss, same-timebase monotonic observations,
bounded 8042/device reinitialization, a source-generation change with sequence
restart, correct final masking, and no error, fatal, unexpected reset, early
exit, or private input in COM1. A normal-image exclusion gate must reject the
named feature-only entry, loss, timer-health, counter, and return-probe helpers,
the event namespace, and the privacy probe. The feature-transcript verifier must
reject every nonempty unstructured line, malformed record, unexpected event, or
unknown field rather than merely searching accepted records for known private
payload shapes.

The dedicated VirtualBox machine must reproduce timer progress, injected
physical transitions, returning execution, and privacy-safe diagnostics. After
the feature run, the normal interactive image must be restored and remain at a
working bootstrap-shell prompt.

Acceptance additionally requires `make check`, the new bounded interrupt gate,
normal-image exclusion, and all existing boot, fault, circle, memory, and shell
regressions.

## Acceptance record

Accepted on 2026-09-09 after host tests, target Clippy, `make check`, all six
QEMU gates, normal-image exclusion, independent review, and dedicated
VirtualBox execution passed. The strict QEMU run reported initial PIC masks
`0xff`/`0xff`, armed masks `0xfc`/`0xff`, and eight PIT interrupts advancing
immutable timebase generation 1 from tick 0 to tick 8. It received 24 ordered
physical transitions at source generation 1 sequences 0 through 23 and returned
with register state preserved after timer and keyboard entry. Deterministic
pressure then produced one exact capacity loss at generation-1 sequence 88,
contained IRQ1 at master mask `0xfe`, completed bounded 8042/device
reinitialization, and rearmed generation 2 at master mask `0xfc`. Injected input
after rearm arrived at generation-2 sequences 0 through 1 before final PIC masks
returned to `0xff`/`0xff`.

The 512 MiB VirtualBox run reproduced that complete lifecycle, with eight PIT
interrupts advancing tick 1 to tick 9 and 130,674 free frames in its feature
image. Neither strict feature transcript contained the private probe or an error-
or fatal-level record.

The normal image was rebuilt after the feature proof. In the dedicated
VirtualBox VM it reported 130,683 free frames, the production `legacy_pc`
readiness state, and bootstrap-shell readiness. Keyboard injection corrected
`versiom` with Backspace plus `n`; the shell accepted the resulting `version`
command through IRQ1, emitted a successful static command classification and
the next prompt, and remains running. The fresh QEMU shell gate reproduced the
same edited command and also passed privacy-safe `echo` and unknown-command
checks. Normal-image inspection rejected all named feature-only helpers, the
`kernel.interrupt_test.` event namespace, and the injected privacy probe.

Independent final review additionally drove and re-audited the forced fatal
record boundary, strict complete transcript schema, fresh empty-IRQ streak after
successful setup or rearm, protocol/error-byte loss handling, exact PIT write
test, and complete normal-image helper exclusion before the final QEMU and
VirtualBox evidence was regenerated from the resulting tree.

## Consequences

The project gains a testable bridge from polling-only bring-up to attributable
external work and monotonic event ordering. The typed, generational boundary
can survive replacement of the legacy controller backend and gives future task,
input-familiar, UI, and machine-control work one debuggable source of truth.

The cost is additional privileged assembly and hardware-controller code plus a
temporary legacy backend that must not become architectural policy. Fixed
buffers make overload visible rather than impossible, and the BSP-only design
does not solve concurrency or scheduling.

## Alternatives considered

- Continue polling the keyboard. This provides neither asynchronous delivery
  nor a time source and keeps device handling entangled with the shell loop.
- Adopt APIC routing immediately. Without ACPI/MADT discovery in `boot_omen`,
  this would bake emulator assumptions into the kernel boundary.
- Reuse the exception dispatcher for external interrupts. Its fatal,
  non-returning contract conflicts with bounded acknowledgement and `iretq`
  return.
- Treat injected keys or COM1 bytes as the AI interface. This erases identity,
  provenance, authorization, privacy, and semantic intent at the most sensitive
  boundary.
