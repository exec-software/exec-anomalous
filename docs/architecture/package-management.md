# Covenant package architecture

Package management is part of the Anomalous system model. Covenant is intended
to manage applications, system vessels, SDKs, models, and environments through
the same inspectable transaction semantics.

This is a proposed design, not implemented code.

The selected Reliquary extension is **`.re`**, under accepted
[ADR 0061](../adr/0061-select-re-extension-and-path-policy.md). The suffix does
not select authority or bypass content validation. Native artifact and future
filesystem contracts must avoid fixed global path-length/depth limits; bounded
transport and resource budgets remain explicit and must never silently truncate
paths. The format encoding and loader remain unimplemented.

## Core model

A **Reliquary** is an immutable, signed, metadata-rich artifact. A
**covenant_manifest** is a declarative statement of desired environment state.
The **Covenant** vessel resolves that statement into a graph and proposes an
atomic transition between generations.

The content-addressed store separates immutable artifacts from mutable state.
Activation changes references to a complete generation rather than editing a
live installation file by file.

## Transaction phases

1. **Resolve** versions, variants, platform constraints, and policy.
2. **Explain** the complete plan, including downloads, removals, sigils,
   services, storage impact, trust changes, and known irreversibility.
3. **Authorize** against policy and explicit human or enterprise gates.
4. **Fetch** from one or more configured archives.
5. **Verify** hashes, signatures, transparency evidence, provenance, and SBOM.
6. **Stage** into an isolated, immutable generation.
7. **Validate** declared health checks without ambient system authority.
8. **Activate** the generation atomically.
9. **Observe** health and offer automatic or operator-directed rollback.

A failed transaction before activation has no visible partial installation.

## Reliquary goals

Reliquary is intended to be both the native executable framing and the package
distribution artifact, not merely a wrapper around a foreign binary. The first
revision may carry conventional architecture-specific machine code while its
native entry records, segment descriptions, semantic schemas, and governed
context mature. Later execution forms must remain expressible without breaking
older readers.

The format should be versioned and chunked so readers can ignore unknown
optional sections. Candidate sections include:

- one or more architecture-specific code payloads;
- semantic object and rite schemas;
- human-facing presentation metadata;
- AI-facing context with explicit ownership and disclosure policy;
- requested sigils and resource intent;
- dependency constraints and compatibility tier;
- cryptographic identity and signatures;
- source and build provenance;
- software bill of materials;
- debug maps, symbols, and crash-unwinding data; and
- deterministic health-check declarations.

Executable code and context have separate trust labels. A valid signature does
not make embedded instructions, model context, or prompts trustworthy.

The kernel should not parse the full format. A constrained loader vessel
validates a Reliquary and passes a compact launch contract across a narrow
kernel ABI.

## Lifecycle behavior

Traditional package scripts often execute arbitrary code with ambient root
privilege. Covenant instead describes lifecycle work as typed rites with
declared inputs, outputs, side effects, network access, and required sigils.
Compatibility hooks run inside a temporary circle and produce an audit record.

User data, secrets, databases, and caches are not blindly rolled back with an
artifact generation. Their migration and compensation behavior must be declared
separately.

## Human and AI clients

The GUI and exec-hag receive the same structured transaction plan. A human may
see an accessible visual explanation while an agent consumes the plan's schema,
but both approve the same digest and policy decision. An AI cannot convert
uncertainty into authority or suppress material effects from the human view.

## Foreign ecosystems

Adapters may ingest packages from Linux distributions, language registries,
Windows installers, or other ecosystems. An adapter is an isolated vessel that:

- resolves foreign metadata;
- captures content into the immutable store;
- produces provenance and an SBOM where possible;
- translates lifecycle work into constrained rites; and
- declares the veil required to execute the result.

Imported packages never become “native” merely by being copied into the store.
Their trust and compatibility origin remains visible.

## Enterprise surface

The open Covenant protocol can support commercial policy archives, signed
release channels, fleet generations, attestations, compliance reporting, staged
rollout, support lifecycles, and managed recovery. Those services do not alter
the MIT rights of the open-source core.
