# ADR 0092: Run NIL on Anomalous with a natively compiled interpreter

- Status: Accepted direction; implementation gated
- Date: 2026-10-15
- Scope: The execution route for NIL (and therefore Matrix) programs on
  Anomalous, and the native build of the NIL interpreter.

## Context

Archon is a Matrix program that scans code and other surfaces for security
issues, and it is intended to become a core operating-system facility. It runs
today only on the host, under the shared Ny runtime's `nyinterpreter`. Running
it — or any NIL/Matrix program — on Anomalous needs an execution route that does
not exist yet: the kernel carries no NIL interpreter, `nycompiler` has no
Anomalous output mode, and its `--native` backend emits a hosted Linux ELF that
is neither the ELF adapter's sectionless deployment subset nor complete enough
for the intrinsics a program like Archon uses.

The one existing route is deliberate and narrow. ADR 0049 qualifies exactly one
bounded *freestanding Ny object*: checked source, assembled NIL, a retained
object, an opaque embedding wrapper and a kernel payload, with no hosted
allocator, TLS, dynamic linker, GOT/PLT or unwinder, and with every import
resolved by inspecting the artifact. It explicitly does not qualify the hosted
Ny runtime or a general compiler target. Native applications are likewise
freestanding `no_std` images with fixed geometry (ADR 0048, ADR 0067).

The alternative route is the interpreter itself. `nyinterpreter` is the public
NIL runtime command; its execution core is the C++ `ny::VM` in the Ny compiler
tree (`code/nycompiler/src/vm.*`) behind a thin `code/nyinterpreter` front end.
It loads and executes any textual or binary NIL edition. That is the most
flexible route: a program needs no per-program native build, and the same
interpreter that already runs on the host becomes the runtime for Matrix work.
But it is a hosted program — dynamic linker, hosted allocation, TLS, operating
system services — precisely the composition ADR 0049 says would "silently exceed
the qualified kernel profile."

This record is required because selecting the interpreter as an execution route
introduces a new platform-level abstraction and a new privilege boundary; an
ADR is required for both (ADR 0000).

## Decision

Select the NIL interpreter as the near-term execution route for NIL/Matrix
programs on Anomalous. Build the interpreter as a native Anomalous task that
targets the Anomalous ABI, so a NIL edition is loaded and interpreted by that
task instead of being compiled ahead-of-time per program.

Defer per-program native compilation. The freestanding-object route (ADR 0049)
is retained but not pursued now: it needs a backend that emits the adapter's
deployment subset and supports the intrinsics programs actually use, and it
duplicates a build per program. **Note for follow-up: revisit native (AOT)
compilation once the interpreter route works; this record is where that intent
lives.**

The interpreter build must be bounded in the sense Anomalous already applies to
Ny objects and native applications: no hosted dynamic linker, no hosted
allocator, no TLS, no unwinder, an inspected instruction profile, fixed low-
address geometry, and only admitted services. The interpreter's operating-system
needs — console output, bounded text/file reads, time, and later richer services
— cross the supervised boundary as explicit services through the shared semantic
control plane (ADR 0005, ADR 0018, ADR 0019); they are not satisfied by the host
libc.

This record selects a direction and names the work. It does not implement it.
The native build profile, the interpreter's service boundary, and its authority
model each need their own qualification before anything runs.

## Consequences

- Any NIL/Matrix program runs on Anomalous without a per-program native build;
  Archon, Oculus and later Ny work share one route.
- The interpreter becomes a large, long-lived vessel. Its authority, isolation
  and audit must be defined and attenuated like any other vessel, and its
  resource ceilings must be explicit.
- Interpretation is slower and heavier than AOT execution. The AOT route stays
  available as a follow-up for hot or tightly constrained programs.
- The interpreter's host assumptions (allocation, TLS, syscalls, threading,
  diagnostics) become a concrete requirements list that drives further runtime
  and service qualification, and it may prove simpler to build a bounded
  interpreter subset than to admit the whole host program.
- Ny adoption remains gated by ADR 0016 and the official language direction of
  ADR 0045; this route does not by itself qualify Ny for privileged migration.

## Alternatives

- **Per-program AOT native compilation (ADR 0049).** Retained, deferred. It is
  the smaller per-program proof but the less flexible route, and the backend gap
  (deployment-subset emission and I/O intrinsics) is real.
- **Run the host Linux interpreter through the Linux veil (ADR 0073).** Rejected
  for NIL here. It would make the interpreter a Linux compatibility payload
  rather than a native Anomalous task and inherit the Linux personality the
  project deliberately isolates (ADR 0008, ADR 0073).
- **A NIL loader or verifier in the kernel, or a new privileged service.**
  Larger new surface than an interpreter vessel, with admission and instruction
  safety concerns of its own; not selected now.

## Follow-ups

- Note: add native (AOT) compilation later; the interpreter route is the
  near-term choice.
- Qualify the interpreter's native build profile (bounded, freestanding).
- Define the interpreter's service boundary and authority model.
- Feed the concrete requirements back into runtime qualification as the
  interpreter actually needs them.

Archon's `notebook/Development/2026-10-15 Archon on Anomalous Build Path Note.md`
and `archon/roadmap.md` (Unit 11) reference this record. The implementer-facing
summary with exact ABI numbers and the likely ABI corrections is
`notebook/Development/2026-10-15 NIL Runtime into Anomalous Handoff.md`.
