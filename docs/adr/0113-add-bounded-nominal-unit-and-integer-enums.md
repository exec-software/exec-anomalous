# ADR 0113: Add bounded nominal unit and integer enums

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0102 and 0105–0112
- Scope: Host-only profile 9 for nominal enums with unit or one i64 payload.

## Context

[ADR 0112](0112-add-bounded-module-globals-and-integer-text.md) provides explicit
integer module globals and integer text conversion. The complete
`names-and-case.matrix` and `value-position-then.matrix` artifacts additionally
require nominal enum declarations; the latter constructs, tests and extracts a
single integer payload. Neither target requires generic or nested enums.

The pinned general VM preserves all signed integer bits in enum payloads, but
its permissive tag and extraction operations do not enforce nominal metadata.
Extraction pushes at most the available payload count. Asking a unit value for
one payload therefore leaves no result, which can expose an older operand to
the next instruction. A bounded typed executor needs an explicit contract for
this behavior instead of silently assuming a payload or manufacturing zero.

## Decision

### Select an explicit profile and preserve admission boundaries

Add profile **9 in both existing envelope headers** through `enums::encode` and
`enums::admit`. Inherit profile 8 while preserving all prior profiles' grammar
and tag refusals. Keep version 1, canonical extents, reserved zeroes, exact
edition hashing, immutable validation witnesses and complete validation before
publishing any output. Do not fall back to a different profile after refusal.

Add `conformance::validate_enums` under the existing opt-in host-only feature.
Its separate raw witness records exact edition bytes and the selected profile;
it cannot become a production witness. Only absence of authored main intent is
relaxed. Present annotations, resource limits, whole-file syntax and references,
independent function CFG checks and the trusted authority ceiling remain required.

### Declare a bounded exact nominal vocabulary

Require a single `# Enum definitions` section marker when declaring enums,
before those declarations and before any function. Editions without enum
declarations need no such marker. Admit declarations of this form:

```text
enum app::state
    Idle
    Busy: i64
end enum
```

A variant is either a bare simple identifier or a simple identifier followed
by exactly `: i64`, allowing the existing surrounding whitespace. A unit is
declared by its bare name; explicit `: void`, multiple fields, other payload
types, generics and nested values remain unsupported. Empty enum declarations
are allowed as metadata but provide no constructible variant.

Bound the edition to **64 enum layouts**, **32 variants per layout** and
**256 declared variants in total**. Count unused declarations too. Layout
names use nonempty ASCII identifier segments joined by `::`. Variant names
are simple ASCII identifiers. Layout names and the complete
`<layout>::<variant>` tag each fit **128 bytes**. Keep exact case-sensitive
spelling and declaration order; variant IDs are local to their layout.

Reject duplicate layouts and duplicate variants within a layout. Record and
enum names cannot collide in either declaration order. Reject the same
case-insensitive primitive nominal names already reserved by records.
Additionally reserve any enum name whose terminal segment is `Option` or
`Result`, case-insensitively, including qualified names. Exclude dotted names
and tags. This is an explicit restriction of this bounded profile: the pinned
VM normalizes dots and special Option/Result aliases, while these ordinary
accepted names need no canonicalization. This decision does not redefine those
families in the general language.

Permit the existing plain `@ai_context("...")` immediately before an enum
declaration, with the same uniqueness, content and **512-byte** bound as record
context. It belongs to that enum and never supplies main intent or authority.
Do not accept a function `# declaration_line` annotation on an enum. All nominal
declarations precede functions; existing string and record sections remain
available under their existing ordering rules.

### Resolve exact typed construction, tests and extraction

Add only these operation forms:

```text
NEW_VARIANT app::state::Idle 0 app::state void
NEW_VARIANT app::state::Busy 1 app::state i64
IS_VARIANT app::state::Idle app::state void
IS_VARIANT app::state::Busy app::state i64
UNWRAP_VARIANT 0 app::state void
UNWRAP_VARIANT 1 app::state i64
```

Counts are complete ASCII unsigned decimals, with leading zeroes permitted,
and their value is exactly zero or one. Require the exact owner type and
payload annotation on each instruction. Resolve every layout and complete
variant tag across the whole file, including dead instructions. Constructors
and tests must match a declared variant's payload shape and owner type. An
unwrap's requested shape must occur among that layout's declared variants.
Missing/extra operands, omitted metadata and legacy forms remain unsupported.

Construction consumes zero operands for a unit or one proven integer for an
integer variant, then pushes one enum value. A tag test consumes one enum of
the exact nominal type and pushes integer zero or one. Extraction consumes an
enum of the exact nominal type and produces exactly the requested zero or one
integer value, subject to the runtime shape policy below.

Locals, branch joins, direct and indirect call arguments and results preserve
the enum's nominal type. Different variants of the same enum can join; two
different enum names cannot join even when their declarations have identical
shapes. Ordinary stack copies and aliases retain the same immutable value.
Exact declared enum names are allowed in helper signatures. Preserve exact
callable signature identity using disjoint fixed type codes for records and
enums, while retaining existing i32/i64 distinctions and parameter order.

Enums cannot act as scalar integers, text, arrays, records or callable targets.
Do not add casts, enum arrays, enum record fields, enum globals, mutation,
capture or external handles. Console operations still require proven scalar
integer or text values. Enum instructions derive no service authority; retain
the whole-file union of actual console effects, including dead operations and
unused helper bodies.

### Keep values inline and check exact extraction shape

Store an enum as private immutable nominal-layout ID, variant ID and optional
full signed-i64 payload in existing bounded operand/local/frame storage. Do
not allocate an enum arena or introduce a cumulative construction quota. Values
survive helper return by ordinary copying. Preserve all 64 payload bits without
floating conversion, implicit narrowing or default substitution.

CFG state tracks nominal type rather than a set of possible variants. At each
unwrap, check the actual selected variant's payload arity against the requested
zero or one **before consuming the owner**. Return the structured error
`enum_shape` for either a missing payload or an attempted discard of a present
payload. A unit cannot expose an older operand or caller prefix as a missing
result. An integer variant cannot silently lose its payload through a unit
unwrap. Extraction is by shape, not by a particular tag; multiple integer
variants of the same enum can use the same integer unwrap.

This exact-shape rule is deliberately stricter than compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`, whose extraction pushes only the
available requested prefix. Qualify mismatches as explicit finite-reference
policy contrasts, never as matching VM faults. The opaque typed operand and
metadata restrictions likewise exclude permissive string/tag forgeries.

Retain all existing validation, instruction, fuel, operand, local, frame and
output bounds. Construction of a unit still requires an available shared
operand-stack slot, including in a helper above a preserved caller prefix. Clear
all enum values and local copies through the existing complete execution-entry
and normal/error-return cleanup. A shape fault preserves earlier acknowledged
output; there is no automatic retry or resumable failed workspace.

The core remains `no_std`, allocation-free and unsafe-free. These host-only
structures establish no native object layout, privileged API or service
authority.

## Evidence contract

Preserve all prior proof/oracle sources and rerun every earlier proof suite
unchanged. Add a separate guarded proof using the actual pinned VM's enum
construction, tag testing and extraction. The oracle must not delegate these
semantics to candidate code. Reference timeout or guard exhaustion cannot
establish equivalence; finite-reference policy contrasts remain separate.

Compare exact console bytes and full-i64 results for unit/integer variants,
matching and nonmatching tags, extrema and integers beyond exact floating-point
precision, aliases, joins, direct/indirect arguments and helper returns.
Exercise exact nominal identity, disjoint callable signatures, unused/dead
metadata checks, declaration and combined-tag limits, reserved alias families,
context/main separation, absent unwrap shapes and invalid operand families.

Require explicit runtime tests for both shape mismatches, missing helper
payloads above a caller prefix, partial output before failure, repeated inline
construction beyond old arena caps, highest layout/variant IDs, operand
exhaustion and complete cleanup/reuse after success, invalid limits, fuel,
frame, output and service failure.

Run the unmodified upstream runner and all retained complete artifacts through
separate raw and production lanes. Preserve the 97 compiler calls, 38 roots,
45 interpreter invocations, auxiliary obligations and telemetry exit-only
exception. Keep all 13 prior raw matches. Require exact matches of the complete
`names-and-case.matrix` and `value-position-then.matrix` editions before claiming
increased coverage. Do not remove unused enum declarations, inject intent,
normalize source tags or rewrite instructions to obtain acceptance. Refused
raw executions remain unmet coverage; broader generics, nested aggregates and
other unsupported constructs remain blocked.

Require complete repository checks, independent retained-evidence audits,
native artifact continuity and Linux-loader regression before accepting this
checkpoint. Full interpreter qualification and native NIL execution remain
separate closed gates. This host profile changes no kernel, native admission,
desktop, task-service mediation or Linux compatibility boundary.
