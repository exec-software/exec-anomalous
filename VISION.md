# Vision

## The last operating system is a direction, not a slogan

AI has changed how software is created, operated, and understood. Retrofitting
agents onto interfaces designed only for humans produces brittle automation;
designing only for agents produces systems people cannot trust or enjoy.
Anomalous begins with both participants in the system model.

The “last operating system” is the north star: a durable, open substrate that
can absorb new interaction models, execution forms, hardware, and foreign
software without requiring users to abandon the system underneath their work.
It is not a present compatibility claim.

## One world model, multiple projections

A person may press a control. A shell may issue a command. An AI may submit a
structured request through a model or agent service. All three should resolve
to the same versioned rite and authorization path, producing the same preview,
audit event, result, and recovery options.

No important state should exist only in pixels. No machine interface should
silently possess authority unavailable for inspection by the user.

## The human experience

Anomalous should feel calm, coherent, fast, and intentional. Its visual language
is accessible neumorphism: restrained depth, clear material relationships, and
polished motion without sacrificing contrast or state legibility. The semantic
accessibility tree is a first-class system surface, not metadata reconstructed
after rendering.

## The machine experience

Objects, operations, schemas, costs, side effects, authorization requirements,
progress, cancellation, and results should be discoverable in typed form.
Machine clients must be able to ask what will happen before it happens.
Irreversible or high-impact work must have explicit policy gates independent of
model confidence.

HAG is the compute runtime intended to replace Vulkan. Model and agent services
carry the higher-level semantics above HAG and use the same governed control
plane as human clients. HAG remains above stable kernel mechanisms and cannot
mint authority.

## Native development experience

Antimatter is the official editor, and Ny is the official language direction
for Anomalous and HAG. Native ABI proofs establish execution, ownership and
task-bound services. The first real application target is the existing Linux
Antimatter build running through Anomalous's ELF/Linux compatibility backend,
as clarified in [ADR 0073](docs/adr/0073-run-linux-antimatter-through-the-linux-veil.md).
Antimatter retains its Electron/React/CodeMirror frontend; a native frontend
rewrite or Reliquary conversion is not a prerequisite. The bounded independent
Rust editor remains a native regression baseline. Linux executable loading must
be accompanied by the runtime, syscall, file, process and desktop environment
the application requires. Compare the actual application with the same Linux
build on the host outside the VM. Windows and macOS comparisons come later.
Software rendering can precede HAG acceleration.

[ADR 0045](docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md)
accepts the product direction. Existing verified Rust remains authoritative
until independently qualified replacements meet each component's requirements.
This direction does not establish Linux Antimatter compatibility, a freestanding
Ny runtime or HAG readiness. The
[Native Antimatter Direction Log](notebook/Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
records the current source evidence and planned progression.

## Native execution and governed context

The Reliquary format will treat code, metadata, context, provenance, signatures,
debug information, and requested sigils as parts of one governed artifact.
Context is data with ownership, lifecycle, and disclosure rules—not an
unbounded prompt attachment.

The format begins as an evolvable container around well-understood payloads.
Novel executable semantics will be adopted only when they demonstrate measured
value and can be tooled, secured, and debugged.

## Storage that keeps identity and history

Files should keep their identity when people or agents rename and reorganize
them. Paths remain useful, but native names, path length and depth have no fixed
global ceiling; chunked storage and incremental traversal account for finite
resources. Versioned operations identify the exact objects and effects involved.

The selected [filesystem direction](docs/architecture/filesystem.md) combines
copy-on-write transactions, verified integrity, snapshots, separately qualified
redundancy and independently restorable backups. Recovery must explain which
state survived without reviving old authority or hiding lost work. This is an
architecture, not an implemented filesystem; the next Antimatter storage gate
is an exact durable save recovered through a fresh service and cold VM boot.

## Package management is part of the operating model

Covenant will compose immutable, content-addressed artifacts into declarative
system and user environments. Installation is an inspectable transaction with
an atomic commit or no visible change. Rollback is ordinary, capability requests
are reviewed before activation, and arbitrary lifecycle scripts receive no
ambient root authority.

Foreign ecosystems can be imported through isolated adapters, but their
provenance and effects must become visible in the Anomalous model.

## Modularity and recovery

The kernel should own only mechanisms that require its privilege: memory
protection, scheduling, interrupts, capabilities, IPC, time, and the minimum
telemetry needed to diagnose failure. Drivers, filesystems, networking, policy,
package management, UI, and compatibility live in supervised vessels that can
be isolated, inspected, replaced, and restarted.

Recovery is a feature. A component should fail with enough structured evidence
to explain what happened, restore service, and improve the system.

## Compatibility without kernel capture

Linux, Windows, and macOS compatibility will be explored in separate veils.
Each veil may use ABI translation, API reimplementation, containers, virtual
machines, or remote execution. The kernel will not become three foreign kernels
interleaved together.

The native ABI supplies the first behavioral baseline for eventual Windows
PE/COFF, macOS Mach-O, and ELF profiles. Each executable container needs an
explicit execution and service personality; ELF alone does not imply Linux,
and a native Ny ELF build input does not prove Linux compatibility. Antimatter
is the intended first real multi-ABI application test, comparing behavior,
ownership, surface/input and workspace services, and cleanup while documenting
each profile's limits. Reading a format alone cannot establish that result.

Legal feasibility is a gate equal to technical feasibility, particularly for
macOS.

## What success looks like

Anomalous succeeds by demonstrating measurable properties:

- boot and recovery are reproducible;
- authority is narrow, visible, and revocable;
- human and machine actions share policy and audit semantics;
- failures produce useful postmortem artifacts;
- package transactions are atomic and explainable;
- the interface remains accessible while visually distinctive;
- foreign applications run with known compatibility and isolation tiers; and
- AI workloads reach hardware efficiently without bypassing safety boundaries.

## Early non-goals

The pre-alpha project does not claim production security, broad hardware
support, full POSIX behavior, complete foreign ABI compatibility, or universal
AI safety. It will earn those claims through isolated milestones and evidence.
