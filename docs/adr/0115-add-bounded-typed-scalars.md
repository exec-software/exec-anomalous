# ADR 0115: Add bounded typed scalars

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-20
- Builds on: ADRs 0102 and 0105–0114
- Scope: Host-only profile 11 for explicit boolean, byte and finite binary64 values.

## Context

[ADR 0114](0114-add-bounded-textual-source-debug-metadata.md) admits descriptive
source metadata without widening executable types. The complete retained
`text-and-types` matrix edition uses boolean, byte and binary64 helper signatures
and explicit casts. Its main function uses already supported integer and text
operations, but validation must still check its complete set of unused helpers.
Removing those helpers would conceal a real language boundary.

The pinned VM represents booleans and bytes as integers at runtime and supports
more scalar conversions than this bounded candidate needs. The next slice must
retain useful declared types through calls, local storage and control flow,
without implicitly widening arithmetic, console output or aggregate contents.

## Decision

### Select an explicit profile and preserve earlier admission

Add profile **11 in both existing envelope headers** through
`typed_scalars::encode` and `typed_scalars::admit`. Inherit profile 10 and keep
every earlier entry point's grammar and profile refusals unchanged. Existing
version 1 framing, canonical extents, reserved zeroes, exact edition identity,
immutable validation witnesses and validation before output publication remain
required. A profile refusal does not trigger a different parser automatically.

Add `conformance::validate_typed_scalars` under the existing opt-in host feature.
It returns the separate raw checked witness and relaxes only missing authored
main intent. Any annotation present must retain its existing grammar and
placement. Complete syntax, references, function CFG validation, effect ceilings
and fixed resource bounds remain mandatory. Raw validation cannot construct a
production admission witness.

### Keep explicit scalar kinds and declared signatures

Add distinct `kind::boolean`, `kind::byte` and `kind::float` values. Admit exactly
`bool`, `u8` and `f64` in helper parameter and result declarations. Preserve them
through direct calls, internal function values, indirect calls, returns, local
stores and CFG joins. The existing `i32` and `i64` operand kind remains integer,
while exact function signatures continue to distinguish their declared widths.

Keep existing exact signature type codes 0 through 133 unchanged. Assign bool,
u8 and f64 codes 134, 135 and 136 respectively. Parameter order, arity, nominal
record and enum identity, and result type remain part of signature identity.
Helpers with the same complete declared signature may share the existing
canonical representative; a different scalar kind must not alias it.

There are no new boolean or byte literal spellings. The pinned textual VM does
not accept `PUSH_CONST true` or `PUSH_CONST false`. Produce boolean and byte
values with an integer literal followed by an explicit cast. Aliases such as
`boolean`, `byte`, `float`, `double` and `f32` remain unsupported. Main retains
its existing zero-argument integer-return contract.

### Admit only the required explicit conversions

Add `operation::scalar_cast` with the public target enum `scalar_cast`:

| Textual target | Target variant | Accepted input kinds | Result and value rule |
| --- | --- | --- | --- |
| `bool` | `boolean` | integer, boolean, byte | Boolean; accept only values 0 or 1 |
| `u8` | `byte` | integer, boolean, byte | Byte containing the low eight bits |
| `i32` | `integer32` | integer, boolean, byte | Integer containing signed low 32 bits |
| `i64` | `integer64` | integer, boolean, byte | Integer preserving the integral value |
| `f64` | `float_identity` | float | Float preserving every stored bit |

A boolean conversion of an integral value other than 0 or 1 is an attributable
runtime `scalar_conversion` failure. Admission checks kinds and does not attempt
constant evaluation to reject `PUSH_CONST 2; CAST bool`. Cleanup and failure
reporting use the existing bounded executor path.

Integer-to-float, float-to-integer, float-to-boolean, float-to-byte and textual
scalar conversions are outside this profile. Existing `CAST text` remains
integer-only. A caller must explicitly convert a boolean or byte to i64 before
using integer operations or integer-to-text conversion. Existing integer
arithmetic, comparisons, branches, negation and logical NOT retain integer
operands. Neither equality nor truthiness implicitly erases the new kinds.

Console output remains integer or text. Arrays retain their existing integer
and text elements; indices remain integers. Global slots, record fields and enum
payloads retain their existing i64 declarations. There is no implicit boolean,
byte or float aggregate conversion. Adding scalar types grants no new service.

### Parse finite decimal binary64 literals and preserve their bits

In profile 11, `PUSH_CONST` first excludes an optional-sign `0x` or `0X` prefix
from floating dispatch. Hexadecimal integer digits may contain `e` or `E` and
must retain the existing signed-i64 parser. A remaining token is considered a
floating spelling only when it contains `.`, `e` or `E`.

Require this complete ASCII decimal grammar, where `DIGITS` means one or more
ASCII decimal digits:

```text
[+-]? (DIGITS ('.' DIGITS*)? | '.' DIGITS+) ([eE][+-]?DIGITS+)?
```

The grammar notation is descriptive. Require at least one mantissa digit and,
when an exponent marker occurs, at least one exponent digit. Decimal points
may have digits on either side, so `.5`, `1.`, `+1.0` and `-1E+0` are valid.
Tokens with neither point nor exponent use the unchanged integer parser;
in particular, decimal `+1` remains unsupported. Reject hexadecimal floats,
NaN, infinities, suffixes, locale separators, numeric separators and incomplete
spellings. No whitespace may occur inside a literal token.

A syntactically valid zero mantissa produces exact signed zero regardless of
the exponent's length or magnitude. Preserve the sign bit of `-0.0`, including
zero spellings with exponents too large for a host integer. Still require a
well-formed exponent; the zero rule does not excuse malformed tokens.

For a nonzero mantissa, use the finite binary64 result of decimal conversion.
Accept representable subnormals and correctly rounded finite values. Refuse
overflow to infinity and nonzero underflow rounded to zero. Preserve rounding
at halfway cases, the minimum subnormal, the normal/subnormal boundary and the
maximum finite value in exact-bit evidence against the pinned helper. There is
no new token-specific limit beyond the inherited 65,536-byte complete edition
bound.

Malformed floating grammar produces the existing unsupported error; valid
decimal spellings outside the accepted representable range produce the
existing syntax error. Invalid UTF-8, whole-edition control checks and edition
capacity retain their earlier errors. Neither encoding nor admission publishes
a partial plan or output on failure.

Store a validated literal as `operation::float(u64)` containing its exact
IEEE-754 binary64 bits. The executor stores and transports the same bits and
performs no floating arithmetic. Identity casts, locals, direct or indirect
calls and returns must preserve the bits, including negative zero. No public
runtime observation privilege or float console formatter is added.

### Keep storage, metadata and authority bounded

Use the existing fixed plan, operand, local and frame capacities. The core
remains `no_std`, allocation-free and unsafe-free. Float conversion occurs
during admission; execution only moves validated bits. Existing instruction
and transfer budgets, cleanup, immutable plan borrowing and caller-owned
encoding buffers remain in force.

Inherit source-debug records without changing their grammar, indexing or
read-only access. A descriptive lexical type string cannot change an operand
kind. Metadata neither consumes a runtime instruction nor grants authority.
Whole-file syntax and reference checks include dead instruction regions, and
all function bodies are independently checked even when main never calls them.
Dead console instructions still contribute to the effect ceiling.

### Distinguish candidate policy from the pinned VM

The reference remains compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. Its
`numeric_literal.hpp` provides the decimal lexical and finite conversion rules,
including the signed-zero exception for arbitrarily large valid exponents.
Its actual VM provides strict integral 0/1 boolean conversion, low-eight-bit
byte conversion and signed-low-32-bit i32 conversion.

The reference erases boolean and byte values to its integer runtime variant.
Distinct candidate kinds and rejection of implicit integer use are deliberate
static policy. The reference also supports scalar text conversions and broader
integer/float conversions; those paths remain unsupported here. These are
finite-reference policy contrasts, not claims that the reference is defective.
No accepted candidate behavior may silently disagree with the pinned VM.

## Evidence contract

Preserve the twenty-one prior proof/oracle source files and rerun all earlier
proof suites unchanged. Add an independently guarded typed-scalar differential
proof using the actual pinned decimal helper and actual VM. Compare immutable
candidate `f64_literals` snapshots with pinned-helper bits. Also observe actual
VM values at the next instruction callback so a matching parser snapshot alone
does not stand in for execution. Private executor tests inspect values carried
through helpers, returns and identity casts before cleanup.

Exercise exact bit boundaries, signs, exponent grammar, zero exponent extremes,
integer radix isolation, boolean failures, byte and width conversions, helper
arity and signatures, indirect calls, CFG joins, local reassignment, aggregate
and console exclusions, complete unused helpers, metadata preservation,
production intent, effect ceilings and atomic encoding. Separate accepted
semantic matches, static policy contrasts, validation refusals and matching
runtime faults. Guard failures or exhausted budgets do not prove equivalence.

Run the unmodified upstream runner and retain all complete captured artifacts.
Keep separate raw and production lanes, the 97 compiler calls, 38 roots,
45 interpreter invocations, auxiliary obligations and telemetry exception.
Preserve all sixteen previous raw matches. Require complete invocation 42,
`text-and-types`, including its unused typed helpers, before claiming an
additional raw invocation and root. Do not strip helpers, rewrite instructions,
normalize source metadata or inject authored intent. Production intent refusal
remains distinct from raw language coverage.

The complete repository validation set, independent evidence audits, native
artifact continuity and pinned Linux-loader regression pass: all 53 fresh jobs
on 735 unchanged source files. All twelve language suites pass 1,216 boundary
cases; all 356 oracle checks pass. The new proof establishes 17 of 45 raw corpus
matches and 11 of 38 roots while all 45 production attempts refuse. The
[Typed Scalars Log](../../notebook/Development/2026-11-20%20Typed%20Scalars%20Log.md)
records final hashes, counts and retained evidence. Full corpus qualification
and native NIL execution remain separate closed gates. This host profile
changes no kernel, native admission, desktop or Linux guest ABI contract.
