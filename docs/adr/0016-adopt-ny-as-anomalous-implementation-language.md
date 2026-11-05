# ADR 0016: Adopt Ny as the Anomalous implementation language

- Status: Proposed
- Date: 2026-09-09

Update, 2026-10-03: [ADR 0045](0045-select-native-antimatter-ny-and-hag-direction.md)
accepts Ny as the official language direction. This proposal's technical
adoption and privileged-migration gates remain open; the capability inventory
below describes the September 9 checkpoint, not the current host toolchain.

If accepted, this record supersedes only the primary-language portion of
[ADR 0003](0003-rust-assembly-and-code-style.md). Its requirements for narrow,
documented assembly and uniform project style remain in force. Existing
accepted Rust evidence remains valid and maintained until an independently
tested Ny replacement exists.

## Context

Anomalous treats people and machines as coequal clients, context as governed
data, diagnostics as part of the runtime contract, and native artifacts as more
than opaque executable bytes. Ny is being built as a high-performance general-
purpose systems language around the same concerns. Its annotations, context
embedding, structured runtime logging, NIL intermediate representation, and
interpreted and native routes make it a strategically aligned implementation
language and a useful target for machine-authored code.

ADR 0003 selected Rust for the first boot and kernel proofs because it provided
the low-level control and tooling available at bring-up. Those proofs are now
valuable evidence. Rewriting them merely to change source language would throw
away review history while expanding the privileged failure surface.

Ny is itself pre-alpha. Its current toolchain does not yet provide every
fixed-width byte, file I/O, native aggregate, pointer, result-propagation,
freestanding ABI, inline-assembly, debugger, or standard-library guarantee that
Anomalous needs. A language designation cannot turn partial behavior into a
security or performance claim. Adoption therefore needs explicit gates and a
reproducible SDK contract.

## Decision

### Language direction

Designate Ny as the proposed primary implementation language for new Anomalous
user-space components and, after the gates below, for systems components where
it can express and verify the required ownership and ABI invariants.

The first production-shaped proving project is
[Eidolon](../../eidolon/README.md), the proposed lossless framebuffer-trace
codec in ADR 0017. It begins outside every privileged boundary and exercises
checked bytes, bitwise operations, temporal state, file streams, pointers,
native code generation, diagnostics, AI-oriented annotations, and eventually
constrained assembly against an independent oracle.

The working transition rules are:

1. Existing Threshold and kernel Rust remain authoritative while their Ny
   prerequisites and replacement evidence are absent.
2. New host tools and unprivileged vessels may adopt Ny one bounded vertical
   slice at a time after both interpreted NIL and native routes pass the same
   externally checked behavior.
3. Ny enters Threshold, the kernel, a loader, or a recovery boundary only after
   a separate ADR specifies its freestanding runtime, ABI, panic/failure model,
   allocation behavior, unsafe boundary, generated-code inspection, and
   emulator evidence.
4. Assembly remains exceptional. Each routine documents inputs, outputs,
   clobbers, architecture and feature requirements, ownership, fallback, and
   tests. A portable Ny implementation remains the conformance oracle.
5. Broad safety, performance, portability, and AI-productivity claims require
   measurements on named workloads and routes.

Ny source follows Anomalous `snake_case` identifiers for project-defined names.
Required external ABI spellings remain unchanged.

### Human and AI language surface

Ny's human source and machine-oriented metadata use the same compiler,
authorization environment, and evidence gates. Machine-authored source or
annotations receive no implicit trust, authority, optimization permission, or
waiver from review.

Executable annotations and embedded context are separately governed data. They
have explicit schemas, size limits, ownership, provenance, disclosure labels,
retention, and stripping behavior. A valid executable signature does not make
embedded prompts or context trustworthy. A future Reliquary may carry Ny code
and context as independently labeled sections; raw context must not leak into
diagnostics merely because a debug mode is enabled.

Compiler and runtime logging uses structured, attributable records with safe
human projections. Private source, paths, pixels, inputs, and embedded context
are absent by default. A human and a machine client see the same build or
execution outcome even when their presentation differs.

### Project-wide SDK contract

Anomalous consumes one complete, generated Ny SDK, shared by every component
that needs it. Anomalous stores immutable generations under
`.runtime/store/versions/<manifest-digest>/` and activates the shared runtime as
a real `runtime/` directory that `eidolon/`, `oculus/`, and future sub-projects
consume directly. Generated SDK payloads are not committed to Git.

Every activated runtime has this root layout, with a platform executable suffix
only where the host requires one:

```text
runtime/
├── nycompiler
├── nyinterpreter
├── nyassembler
├── nylinker
├── nylsp
├── nydebugger
├── nyschema
├── nyagent
├── nylib/
├── nystd/
├── docs/
├── manifest.json
└── LICENSE
```

The command executables live directly at the SDK root. There is no `bin/`
indirection or retired runtime compatibility alias. `nyinterpreter` is the
sole canonical runtime name.

`nylib/` mirrors every standard-library module as a checked binary NIL build.
`nystd/` carries the corresponding Ny source. `docs/` carries compiled
documentation. `manifest.json` records the exact source revision, dirty state,
host/build configuration, layout, file sizes, and content digests.

SDK refresh is failure-atomic. The Ny build, tool tests, strict standard-library
compilation, documentation build, manifest, layout validation, and an SDK-local
compile/interpreter smoke test all pass in staging before activation changes.
An unchecked standard-library artifact or `--no-typecheck` fallback is not an
SDK. A failed refresh leaves the prior complete generation active.

`scripts/refresh-ny-runtime.sh` invokes the sibling Ny repository's SDK builder
with the shared Anomalous destinations and revalidates this contract before it
copies the resolved generation into `runtime/`.

### Adoption gates

Before Ny is considered primary in an accepted revision of this record, the
project requires:

- a reproducible SDK with every standard-library source checked and mirrored in
  `nylib`;
- deterministic compiler, assembler, linker, interpreter, and native-route
  conformance vectors;
- exact fixed-width integer and byte behavior across arrays, pointers, NIL,
  files, and native ABI calls;
- structured errors and cleanup for allocation, I/O, parsing, linking, and
  runtime failure;
- a debugger and symbols sufficient to attribute native failures to Ny source;
- sanitizer, mutation, fuzz, and generated-code inspection appropriate to each
  component boundary;
- Eidolon cross-implementation evidence described by ADR 0017; and
- a separately accepted freestanding ADR before privileged migration.

## Required invariants

- A language name, annotation, source signature, or AI origin never confers
  authority.
- Existing accepted behavior is replaced only after equivalent or stronger
  tests pass; migration never silently weakens a gate.
- Interpreted success does not imply native correctness, and native success
  does not excuse interpreter regressions.
- Compiler, linker, runtime, SDK source, standard-library binaries, and docs are
  attributable to one manifest.
- SDK activation exposes either one complete generation or the previous one,
  never a partially copied tree.
- Rich source, metadata, debug, and artifact parsing remains outside the kernel
  unless a narrow independently specified representation is required there.
- Embedded context has independent trust and disclosure policy from code.
- Unsafe memory and assembly boundaries are explicit, bounded, inspectable, and
  backed by portable tests.

## Non-goals

This proposal does not:

- rewrite the current boot loader or kernel;
- claim that Ny is presently production-safe or faster than Rust or C++;
- stabilize Ny syntax, NIL, ABI, SDK manifest, or Anomalous artifact formats;
- place a language server, compiler, parser, or standard library in the kernel;
- make AI-generated code trusted or self-authorizing;
- merge Ny with the proposed Canticle command language.

Canticle remains a proposed capability-explicit command language and semantic
control-plane client. Ny is a general-purpose systems implementation language;
one does not replace the other's security role.

## Consequences

Anomalous and Ny gain a shared, production-shaped proving loop instead of
isolated language demos. The strict SDK and multi-route gates make missing
semantics visible early and give people and machine contributors the same
reproducible tool surface.

The project also accepts bootstrap and migration cost. For a time, Rust and Ny
will coexist, test matrices grow, SDK generations consume local storage, and
the Ny compiler becomes a high-impact supply-chain input. Avoiding a rewrite
keeps that cost bounded while evidence accumulates.

## Alternatives considered

### Rewrite the demonstrated kernel immediately

This would maximize Ny coverage but discard accepted evidence and combine
language bring-up with privileged architecture changes. User-space proofs close
toolchain gaps with less system risk.

### Keep Rust permanently primary and use Ny only for examples

That would not exercise the freestanding, artifact, context, observability, and
AI-oriented design goals that align the projects. A gated transition tests the
claim without pretending it is already complete.

### Commit one SDK snapshot to the repository

This makes review and storage noisy, obscures source attribution, and becomes
stale. Content-addressed local generations plus a strict refresh script preserve
reproducibility without treating binaries as source.

### Keep tools under `sdk/bin`

The extra lookup contract caused naming and discovery drift. A flat root makes
the SDK self-contained and matches the explicit public Ny tool names.
