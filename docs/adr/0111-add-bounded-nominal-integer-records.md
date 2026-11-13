# ADR 0111: Add bounded nominal integer records

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0102 and 0105–0110
- Scope: Host-only profile 7 for nominal records with declared i64 fields and shared mutation.

## Context

[ADR 0110](0110-add-bounded-internal-callable-values.md) provides bounded
internal function values and indirect dispatch. The smallest remaining complete
record artifacts require named schemas, nominal helper parameters, construction,
field reads and field writes. One artifact also carries record-scoped context.

The pinned VM's record copies share storage: local aliases, helper parameters
and returned values observe the same mutations. An immutable record or a copied
field array would produce different results. Its general record implementation
also accepts behaviors outside a typed subset, including undeclared fields.
Admission must establish layout membership and nominal identity explicitly.

Keep the extension separately selected, preserve earlier profiles and separate
record metadata from authored main intent. Nested records, text fields, arrays
of records, classes, pointers and general heap management remain later work.

## Decision

### Select profile 7 and preserve both witness boundaries

Add profile **7 in both existing envelope headers** through `record::encode`
and `record::admit`. Preserve version 1, canonical extents, reserved zeros,
hashing, complete validation before output publication and the immutable
production witness. Earlier profiles retain their grammar, call resolution and
tag refusals. Profile 7 rejects every other tag without parser fallback.

Add `conformance::validate_record` under the opt-in host-only feature. Its
distinct raw witness records the exact bytes, selected profile and authority.
Raw validation permits absent main intent; production still requires valid
authored main intent. Present metadata remains fully checked. No raw witness
becomes a production package or grants a native capability.

### Declare closed nominal layouts before functions

Profile 7 inherits profile 6 and adds one optional `# Struct definitions`
section marker. Every record declaration requires that marker and precedes all
function declarations. String constants may precede the section, as in the
pinned compiler output. Record declarations do not use `# declaration_line`.

The supported shape is `struct <name>`, zero or more `<field>: i64` lines, and
`end struct`. Empty records are valid. Names are simple ASCII identifiers or
nonempty simple identifier segments joined by `::`, with at most 128 total
bytes. Fields are simple identifiers with the same byte bound. Keep exact case
and qualified spelling as nominal identity. Duplicate record names or fields
within one record refuse. Identically shaped records with different names are
different types.

Reject the pinned nominal validator's complete reserved-name vocabulary,
case-insensitively over the whole name: `void`, `any`, `anything`, `bool`,
`boolean`, `flag`, `char`, `text`, `string`, `str`, `int`, `float`, `double`,
`i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `isize`,
`usize`, `pointer`, `array` and `slice`. This avoids collisions with primitive
runtime behavior. A qualified name such as `geo::bool` is not that reserved
whole name. Non-i64 fields, nested fields, generic schemas, inheritance and
other declaration forms remain unsupported.

Bound schemas to **64 layouts**, **32 fields per layout**, and **256 total
declared fields**. Reusing a field name in another layout still consumes one
declared-field slot. Intern distinct field names into at most 256 global names;
each layout retains their byte-sized IDs in declaration order. Runtime and CFG
checks use these resolved IDs without parsing names during execution.

### Keep record context separate from main intent

An exact `@ai_context("plain text")` immediately preceding a record attaches
only to that record. Use the existing nonempty plain-text grammar and 512-byte
limit; escapes, control characters, malformed shapes and duplicate pending
annotations refuse. Each record may have its own context. Expose that context
as read-only layout metadata.

Profile 7 holds context pending until the following supported declaration
determines its scope. Record context never populates main intent, survives as
main intent, or grants service authority. A separate annotation before main
still supplies production intent. An annotation on another function remains
unsupported by the accepted intent policy. Raw conformance preserves record
metadata while reporting absent main intent as absence.

### Preserve nominal kinds through fields, calls and control flow

Add exact one-operand instructions:

- `NEW_STRUCT <declared name>` creates a fresh record of that nominal layout.
- `GET_FIELD <declared field>` consumes the record and returns an integer.
- `SET_FIELD <declared field>` consumes an integer value above its record,
  mutates that field and leaves the same record on the stack.

Resolve every constructor name and field name across the whole artifact,
including dead instructions. A field name must appear in at least one declared
layout. For every reachable field operation, the CFG must additionally prove
that its receiver is a record whose particular layout contains that field.
Dead code receives no invented receiver or operand state, following ADR 0105.
Unknown constructors or globally unknown fields still refuse when dead.

Represent record values as `record(u8)` abstract kinds carrying nominal layout
IDs. Helpers may take and return those exact declared types; locals, branches,
loop joins and direct/indirect calls retain nominal identity. Different layouts
cannot merge or substitute for each other even when their field names match.
`main` remains zero-argument and integer-returning.

Replace the private packed signature key from ADR 0110 with a fixed sequence of
eight parameter type codes plus one result code, compared with the existing
arity. Retain distinct i32/i64 codes and add disjoint nominal layout codes.
Canonical callable signature IDs remain byte-sized first-matching declaration
indices. This representation changes no earlier grammar, signature equivalence
or wire layout and keeps abstract kinds compact in the fixed CFG workspace.

Records cannot act as integers, text, arrays or callable values. Console,
arithmetic, comparisons, casts, conditions, array construction and concatenation
retain their existing type requirements. Record pointers, reference wrappers,
record arrays, callable parameters/results and captures remain unsupported.
No constructor or field operation derives additional service authority; the
whole-file union still includes console operations in unused functions and dead
instructions.

### Own mutable record storage for the complete execution

Follow compiler/VM revision `4dad0512dfd3fa4e351f2816196cb48956f5442e`:
`StructValue` copies share record storage; `SET_FIELD` retains its owner;
`GET_FIELD` consumes its owner; an absent scalar field reads as integer zero.
Because every admitted field is declared i64, allocate a zeroed cell for every
layout field and preserve complete signed-i64 values on writes and reads.

Use private handles into an execution-owned arena bounded to **128 record
descriptors** and **1,024 i64 field cells**. Each descriptor carries its nominal
layout and contiguous field extent. Empty records consume a descriptor and no
cells. A new record is fresh even when its layout matches an existing record.
Aliases, arguments and returns copy handles, preserving shared mutations across
helper returns and local reassignment.

Allocation is cumulative during a run, without per-frame reclamation, freeing,
garbage collection or descriptor reuse. Preflight layout, result-stack capacity
and both storage limits before publication. Exhaustion returns `record_limit`
without publishing a new handle or making a sink call for that instruction.
Validate receiver identity, field membership and the integer assignment before
mutating a cell. Operations cannot cross the current call frame's stack floor.

Retain existing frame, operand, instruction, validation, array, computed-text
and console limits. Clear all record descriptors and field cells, allocation
counts, operands, locals and existing arenas at entry and every normal/error
return. Earlier acknowledged sink effects remain visible after later failure;
there is no retry or rollback. Handles cannot escape synchronous execution.
The core remains `no_std`, allocation-free and unsafe-free. These host bounds
establish no native stack or mapping geometry.

## Evidence contract

Preserve earlier profiles, proof suites and reference sources unchanged. Add a
separate guarded record proof against the actual pinned VM. Its permissive
record maps do not establish the candidate's admission policy: independently
check schema uniqueness, nominal signatures, exact i64 fields and receiver
membership before invoking the oracle. Reference timeout or guard exhaustion
cannot establish equivalence.

Compare exact full-i64 results, errors and console bytes for zero defaults,
field order, extreme values, repeated mutation, aliases, helper mutation and
return lifetime, direct/indirect record calls, fresh allocations and later
faults. Exercise empty records, exact descriptor/cell limits, allocation
atomicity, caller-prefix isolation and cleanup across successful and failed
executions. Keep finite-reference resource contrasts separate from matches.

Require refusal coverage for duplicate/unknown layouts and fields, malformed
names, reserved primitive names, unsupported field/signature kinds, nominal
argument/result and CFG mismatches, scalar/array/function misuse, incomplete
declarations, oversized layouts and invalid record context. Confirm dead-name
checks, whole-file authority, earlier-profile refusals, exact signature identity
and the independence of record context from production main intent.

Run the unmodified upstream runner and complete retained artifacts through
separate raw and production lanes. Preserve all 97 compiler calls, 38 roots,
45 interpreter invocations, auxiliary obligations and the existing telemetry
exit-only exception. Refused raw executions remain unmet semantic coverage.
Do not inject main intent, remove record context, rewrite fields or strip
declarations to obtain acceptance.

Require complete repository checks, independent retained-evidence audit, native
artifact continuity and Linux-loader regression before accepting this checkpoint.
Full interpreter qualification and native NIL execution remain separate closed
gates; this host profile changes no kernel, native admission, service mediation
or Linux compatibility boundary.

## Qualified checkpoint

All 41 final jobs pass on 705 unchanged non-Markdown source files, with an
independent aggregate audit. The new 115-case proof records 42 exact semantic
matches, one matching fault, five finite-reference policy contrasts and 67
validation refusals. It retains 48 reference runs, 115 production checks and
48 prior-profile checks, plus three prior-profile corpus comparisons. All eight
proof suites pass their combined 651 cases, with 72 separate oracle guards.

Raw corpus coverage reaches 12 of 45 interpreter invocations and seven of 38
roots. All 45 production invocations still refuse: 21 unsupported, nine edition
and 15 intent refusals. Native admission artifacts remain byte-identical; fresh
TCG regression passes, and Linux retains 12 requests, 11 returns and its blocked
sequence-12 `mmap` boundary without exit. Full interpreter qualification and
native execution remain false. The
[Nominal Records Log](../../notebook/Development/2026-11-13%20Nominal%20Records%20Log.md)
retains exact source/evidence identities, independent audits, the reviewed
allocation preflight correction and the remaining gates.
