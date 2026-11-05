# ADR 0003: Use Rust with contained assembly and a uniform style

- Status: Accepted
- Date: 2026-09-08

The primary-language designation below is superseded by
[ADR 0045](0045-select-native-antimatter-ny-and-hag-direction.md) as of 2026-10-03.
The contained-assembly, unsafe-boundary, and style decisions remain accepted;
existing verified Rust remains authoritative pending qualified replacements.

## Context

Kernel development needs low-level control, explicit invariants, and code that
people and machine tooling can navigate consistently.

## Decision

Rust is the primary implementation language. Machine code or assembly is used
where firmware, CPU transition, interrupt, context-switch, or measured hot-path
requirements justify it. Unsafe Rust and assembly remain narrow and document
inputs, outputs, clobbers, alignment, ownership, privilege, and validation.

Project-defined code identifiers use `snake_case`, including types, variants,
and constants. Rust and C-family braces follow K&R placement with four spaces.
External interfaces retain required upstream spelling.

## Consequences

The code intentionally allows Rust naming lints that conflict with this project
style. Review and tests carry the burden at unsafe and assembly boundaries.
