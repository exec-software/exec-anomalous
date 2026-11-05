# ADR 0010: Make Canticle a typed, capability-explicit command language

- Status: Proposed
- Date: 2026-09-08

This record proposes the Canticle name, surface, and machine representation;
they are not stable while its status remains Proposed. Existing requirements
that people and machines share one authorization path and that clients cannot
mint authority remain project constraints independently of this syntax.

## Context

Anomalous needs a command environment that people can explore interactively and
machine clients can construct without translating intent through terminal text.
Traditional shells provide valuable ergonomics, but implicit word splitting,
stringly typed pipelines, ambient process authority, dynamic evaluation, and
status-code conventions make effects difficult to inspect or authorize.

A machine-only document format would avoid some parsing hazards while imposing
an unnecessarily hostile human interface. Separate human and machine command
systems would also violate the shared semantic-control-plane decision in
[ADR 0005](0005-shared-semantic-control-plane.md).

## Decision

The proposed Anomalous command language is named **Canticle**. Human-readable
source and a deterministic machine form represent the same versioned, typed
abstract syntax tree. Both pass through identical parsing or decoding limits,
name resolution, type checking, effect checking, rite-schema resolution,
policy, and audit paths. Machine clients may construct the tree directly; doing
so grants no trust or authority advantage.

Canticle has the following foundational rules:

- values and pipelines are typed and structured rather than implicitly split
  text;
- bindings are immutable by default, and core evaluation has no ambient
  environment, current-process authority, executable search path, or inherited
  file descriptors;
- system observations use an explicit `invoke` form;
- state changes use distinct `preview` and `commit` forms, with the commit bound
  to the preview digest and resolved rite schemas;
- every system operation names the sigil bindings presented to it with a
  `with` clause; declarations and names never mint authority;
- strings never undergo implicit interpolation, command substitution, globbing,
  option parsing, or reparsing as source;
- the core language has no general `eval`, implicit executable fallback, or
  automatic conversion between text, paths, bytes, numbers, booleans, secrets,
  and object references;
- fallible work returns typed results, and unhandled pipeline failure cancels
  upstream work and remains visible; and
- time, randomness, network access, secret access, process creation, and other
  nondeterministic or externally visible behavior are rites rather than hidden
  language primitives.

The source grammar uses ASCII `snake_case` identifiers, explicit function-call
arguments, `|>` for structured pure transformations, and conventional K&R
braces. Unicode is permitted in string data but not in identifiers. The first
machine representation is a canonical JSON encoding of explicitly tagged AST
nodes; an eventual compact wire encoding must round-trip the same logical tree
and conformance vectors.

Canticle is a client language, not the policy engine or a sandbox. The semantic
control plane validates sigils and enforces policy even after Canticle has
statically checked a program. Commit authorization is bound to the effects
represented by a preview digest, never arbitrary source text or an AST hash.

The early kernel-resident bootstrap interpreter is not Canticle. It is a
temporary, bounded bring-up interface with exactly `help`, `version`, `about`,
`clear`, and `echo`. It has no variables, pipelines, filesystem, network,
dynamic evaluation, or authority model. It remains in ring 0 only until circles,
IPC, and vessel launch exist. The full parser, compiler, session state, and
semantic command environment will run in an unprivileged Canticle shell vessel.

The detailed proposed syntax, type model, effect rules, machine form, threat
boundaries, and implementation slices are recorded in
[the Canticle architecture](../architecture/canticle.md).

## Consequences

The shell can render rich values for people while agents consume their schemas,
and both clients can produce identical plans and audit records. Injection and
confused-deputy risks have fewer implicit conversions to exploit. Programs can
be formatted, inspected, budgeted, and compared before execution.

The design requires a real parser, formatter, type and effect checker, canonical
encoder, schema negotiation, and structured renderer before it can replace the
bootstrap console. Familiar POSIX idioms will require explicit compatibility
adapters. Capability-explicit commands are somewhat more verbose, so discovery,
completion, local bindings, and excellent rendering are product requirements.

No production-security claim follows from the language design. Parsers,
renderers, policy services, vessels, compatibility boundaries, and the kernel
remain independently hostile-input boundaries that require tests and review.

## Alternatives considered

### Adopt a POSIX-compatible shell as the native command model

This would provide immediate familiarity and a large script ecosystem, but it
would make strings, processes, environment inheritance, and ambient authority
the native abstractions. POSIX tools can remain available inside compatibility
veils without defining Anomalous semantics.

### Use only JSON or another machine document syntax

An explicit data format is easy for machines to construct but poor for direct,
exploratory human use. Canticle instead gives a concise source projection and a
canonical AST projection of the same checked program.

### Give human, automation, and AI clients separate command APIs

Specialized projections are useful, but separate authorization semantics would
create inconsistent previews, hidden privilege, and divergent audit records.
All projections therefore resolve to the same rites and invocations.

### Treat natural language as executable source

Natural language can help a client propose a Canticle AST, but ambiguity and
model uncertainty cannot define the authorization boundary. The proposed tree,
resolved effects, sigils, and preview digest remain inspectable before commit.
