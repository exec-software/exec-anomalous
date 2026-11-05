# ADR 0061: Select `.re` and avoid fixed native path limits

- Status: Accepted
- Date: 2026-10-04
- Scope: Naming and design requirements; no serialized format or filesystem ABI is stabilized.

## Context

The user selected `.re` as the short extension for Reliquary and directed that
native development avoid path-length limits. The proposed format in ADR 0006
has no implemented encoding or parser yet. Current native execution proofs
load inspected code segments; their intermediate ELF and raw payload files are
not serialized Reliquaries.

## Decision

1. **Reliquary artifacts use `.re`.** For example, a future native Antimatter
   artifact may be named `antimatter.re`. The suffix is a naming convention,
   never proof of content, validity, trust, or executable authority. Admission
   must validate the eventual format and the requested execution personality.
2. **Native paths have no fixed global length or depth ceiling.** Do not bake
   `PATH_MAX`, fixed path buffers, or an equivalent inherited platform limit
   into Reliquary or future native filesystem/application contracts. Use
   explicit lengths, checked arithmetic, dynamically sized storage or
   incremental traversal, and scoped directory/object handles as appropriate.
   A short suffix is a preference, not a workaround for a native path limit.
3. **Resource accounting remains explicit.** Allocation, work, transport chunks,
   and qualified proof capacities are finite. They must not silently become a
   permanent path limit. Exhaustion returns a structured failure without
   truncation, aliasing, partial activation, or authority expansion. Long-path
   handling must respect cancellation and memory budgets.
4. **Compatibility constraints stay local.** A foreign filesystem or ABI may
   impose its own constraints. Its adapter must report them explicitly rather
   than exporting them as a native system-wide restriction.

## Consequences

Future format and filesystem work needs tests for long and deeply nested paths,
incremental resolution, arithmetic overflow, resource exhaustion, cancellation,
and complete failure cleanup. Exact encodings and transport mechanisms require
their own reviewed decisions. This record implements neither a Reliquary parser
nor a filesystem and does not promise infinite resources.
