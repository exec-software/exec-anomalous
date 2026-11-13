# ADR 0109: Add bounded immutable typed arrays

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0102 and 0105–0108
- Scope: Host-only profile 5 for typed integer/text array literals, lengths and indexed reads.

## Context

[ADR 0108](0108-add-bounded-computed-text-and-hexadecimal-integers.md) adds
computed text and signed hexadecimal integers to the bounded host interpreter.
The pinned compiler's ordinary iteration over literal text arrays additionally
requires typed array construction, `LEN` and `GET_INDEX`. Supporting those
operations requires distinct handle types, exact element kinds and storage that
survives helper returns. Treating a handle as an integer would incorrectly admit
arithmetic, conditions, casts or console formatting over array storage.

Keep this extension separately selected. Existing profile meanings, production
intent policy, full interpreter qualification and native execution gates remain
unchanged. Mutable arrays, general heap allocation and pointers require later
decisions.

## Decision

### Select an explicit profile and retain both validation purposes

Add profile **5 in both existing envelope headers** through `array::encode` and
`array::admit` in `anomalous-program-flow`. Preserve envelope version 1, exact
header layouts, canonical extents, reserved zeros, edition hashing and the
private immutable admission witness. Profile 5 rejects every other tag; earlier
entry points reject profile-5 packages and retain their existing grammar. No
parser fallback or automatic profile selection is added.

Add `conformance::validate_array` under the existing opt-in, host-only
`host_conformance` feature. It returns the distinct raw checked witness recording
profile 5 and the exact held edition. It neither emits a package nor creates a
production admission witness. Production still requires authored main intent;
raw conformance permits only its absence. Present annotations retain all
existing placement, shape, uniqueness, text and size checks.

Both purposes retain whole-file syntax, reference and authority validation,
explicit trusted service ceilings and reachable CFG validation from every
function entry. Unused functions are validated. Dead instructions contribute
authority and receive syntax/reference checks without invented operand states.
No array operation derives new service authority.

### Admit a closed immutable array subset

Profile 5 inherits all profile-4 forms and adds exactly:

- `NEW_ARRAY_LITERAL <count> i64` and `NEW_ARRAY_LITERAL <count> text`.
  The complete count token must be unsigned ASCII decimal, with leading zeros
  allowed and no sign or radix prefix. Its value is at most **256**. Consume that
  many scalar operands and retain their source order. Every consumed value must
  have the declared element kind. Empty literals remain typed arrays.
- Operand-less `LEN`, consuming an integer or text array and producing an
  integer length. Scalar text is deliberately outside this form even though
  the reference VM also supports text length through `LEN`.
- Operand-less `GET_INDEX`, consuming an array followed by an integer index
  and producing its scalar element kind.

Helpers may take and return the exact canonical type spellings `array<i64>`
and `array<text>`. Existing integer/text/void signatures remain supported.
`main` remains a zero-argument function returning `i32` or `i64`. Array aliases,
bracket syntax, `array<i32>`, other scalar element types, nested arrays and
pointer types are unsupported. Integer arrays store full i64 values; this slice
introduces no implicit element narrowing or coercion.

Track `array_integer` and `array_text` as distinct kinds through operand stacks,
locals, joins, parameters and returns. A call or join cannot substitute one for
the other or substitute an array for a scalar. Literal construction requires
homogeneous scalar operands; indexed reads restore the exact element kind.
Preserve definite local initialization, caller stack prefixes and all existing
structural bounds.

Arithmetic, comparisons, casts and branch conditions keep their prior integer
requirements. Text concatenation still requires two text operands. Console
operations explicitly accept only integer or text values; they cannot inherit
generic-pop acceptance of array handles. `POP` may discard an array handle.
Mutation, array extension, array arithmetic, globals, raw pointers, freeing and
other allocation forms remain unsupported.

### Follow the pinned default bounds semantics

Use compiler/VM revision `4dad0512dfd3fa4e351f2816196cb48956f5442e` as the
semantic reference. Its literal constructor pops operands in reverse to preserve
source order and represents arrays with an `ArrayHandle` distinct from integer
pointers. Its default bounds mode returns the declared scalar default for an
out-of-range array read: **integer zero or empty text**. Apply that behavior to
empty arrays, negative indices and indices at or beyond length, including signed
i64 extremes. Never wrap a negative index into a valid element offset.

The reference also has a strict-bounds mode that throws. This profile selects
the pinned default behavior and exposes no ambient environment switch for it.
Reference proofs must explicitly remove the strict-bounds environment override
and record the selected mode; strict-mode faults are not this profile's oracle.

### Own bounded array storage for the complete execution

Add a private array handle and a caller-owned fixed array arena alongside the
computed-text arena. Each descriptor records a contiguous element extent and
its scalar kind. Store literal and computed text values using their existing
representations, preserving single decoding and the lifetime of computed bytes.
An array handle grants no integer pointer operations or external access.

| Resource | Bound |
| --- | ---: |
| Elements in one literal | 256 |
| Array descriptors allocated during one execution | 128 |
| Scalar array elements allocated during one execution | 1,024 |
| Existing operand values across all frames | 256 |
| Existing cumulative computed-text bytes | 65,536 |

Each literal allocates a fresh descriptor, including an empty literal. Empty
literals consume zero element slots but still consume one descriptor. Allocation
is cumulative for the run. Locals, arguments, returns and aliases copy handles;
discarding or overwriting a handle does not reclaim storage. Returning from a
helper never frees descriptors, elements or referenced computed text. There is
no mutation, garbage collection, deduplication or per-frame reclamation.

Before publication, check the complete operand types, caller frame floor,
result-stack capacity and both arena capacities. Allocation preserves source
order and never overwrites an earlier array. Descriptor or element exhaustion
returns `array_limit`, without publishing a result or making a sink call for
that instruction. Earlier acknowledged console effects remain visible. Arrays
still obey existing instruction, frame, stack and console limits.

Clear every descriptor and element slot, both allocation counts, computed text,
frames and operand storage at execution entry and every normal or error return.
Execution is synchronous and no handle escapes its API. These storage bounds
establish no native stack or mapping geometry. The host core remains `no_std`,
allocation-free and unsafe-free.

## Evidence contract

Preserve all earlier profiles, reference sources and proof suites unchanged.
Add a separate array proof using the actual pinned VM, with a guarded oracle
that admits only the intended semantic subset and records default bounds mode.
Reference resource exhaustion or timeout cannot establish equivalence. Keep
finite-reference resource contrasts separate from exact semantic matches.

Compare full-i64 results, exact console bytes and errors for source-order
construction, empty arrays, both element kinds, in-range and out-of-range
indices, helper parameters/returns, aliasing and caller-prefix preservation.
Include text arrays containing escaped literals and computed strings, later
allocations, and failures after previously acknowledged output. Verify both
arena limits, empty-descriptor accounting, atomic allocation failure, bounded
fuel and complete storage clearing across successful and failed runs.

Require hostile validation coverage for mixed and nested values, wrong read
operands, scalar misuse of handles, call/return and CFG kind mismatches,
uninitialized locals, unsupported mutation and pointer forms, malformed counts,
oversized literals, dead syntax/references/effects, exact signature spelling,
both profile tags and prior-profile refusals. Preserve production intent and
ceiling refusals independently of raw semantic validation.

Run the unmodified upstream runner and all complete retained artifacts through
separate raw and production lanes. Preserve all 97 compiler calls, 38 root
fixtures, 45 interpreter invocations, auxiliary obligations and the existing
single telemetry exit-only exception. Count every unsupported or refused raw
execution as unmet semantic coverage. Do not inject intent, strip helpers or
rewrite emitted instructions to obtain acceptance.

Require the repository's complete checks, independent retained-evidence audit,
native artifact continuity and Linux-loader regression before accepting the
checkpoint. Full interpreter qualification and native NIL execution remain
separate closed gates; this slice changes neither the kernel nor native
admission, task-bound services or Linux compatibility boundaries.

## Qualified checkpoint

All 35 final jobs pass on 692 unchanged non-Markdown source files. The new
87-case proof records 36 semantic matches, one matching fault, seven finite
policy contrasts and 43 validation refusals. Eight of 45 unchanged raw corpus
invocations match, including five of 38 roots; all 45 retain production refusal.
The five earlier proof suites pass unchanged. Independent retained-evidence
checks verify exact bytes, full-i64 boundary results, corpus process records,
oracle limits, original-checkout preservation and native artifact continuity.
All required regression checks pass, including the unchanged Linux-loader
boundary. The [Typed Arrays Log](../../notebook/Development/2026-11-13%20Typed%20Arrays%20Log.md)
records exact evidence and remaining gates.
