# ADR 0112: Add bounded module globals and integer text

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0102 and 0105–0111
- Scope: Host-only profile 8 for explicit module i64 globals and integer-to-text conversion.

## Context

[ADR 0111](0111-add-bounded-nominal-integer-records.md) provides bounded shared
records. The complete `two-books.matrix` artifact additionally requires typed
module globals and `CAST text` over integer values. It emits an ordinary
`__globals_init__` helper and explicitly calls that helper from main.

Module state must be shared across the execution's function frames without
becoming host process state. Missing globals cannot silently read as zero, and
integer formatting must produce owned text that survives helper returns. Keep
these additions separate from the reference VM's broader legacy local/global
lookup fallback and other scalar conversions.

## Decision

### Select an explicit profile and preserve admission boundaries

Add profile **8 in both existing envelope headers** through `globals::encode`
and `globals::admit`. Preserve version 1, canonical extents, reserved zeros,
edition hashing, immutable validated plans and complete validation before output
publication. Earlier profiles retain their exact grammar and tag refusals;
profile 8 rejects every other tag without parser fallback.

Add `conformance::validate_globals` under the existing opt-in host-only feature.
Its separate checked witness records the exact edition and selected profile.
Raw validation permits absent authored main intent; production continues to
require it. Present annotations retain their existing checks, and inherited
record context never supplies main intent. Neither raw validation nor a global
slot creates a production package or native authority.

### Admit only explicit typed integer globals

Profile 8 inherits profile 7 and adds exactly:

- `LOAD_GLOBAL <name> i64`, pushing the current integer value of that slot.
- `STORE_GLOBAL <name> i64`, consuming one integer and overwriting that slot.
- `CAST text`, consuming a proven integer and producing text.

The exact `i64` annotation is mandatory on every global access. Legacy untyped
forms, other annotations, extra operands and separate global declaration syntax
remain unsupported. `STORE_GLOBAL` leaves no duplicate value on the stack.
Reachable stores require an integer; text, arrays, records and callable values
cannot be stored, implicitly converted or used to fabricate a callable target.
Existing explicit integer casts retain their prior behavior.

Global names are case-sensitive simple ASCII identifiers, or nonempty simple
identifier segments joined by `::`, with at most **128 total bytes**. Keep their
exact spelling. Generated dollar locals and dotted names are not global names.
Because this is a distinct explicit namespace, primitive words and names shared
with functions, locals, records, constants or console builtins are permitted.

Intern at most **128 distinct global slots per edition** in first textual
access order across the complete artifact. Both loads and stores, including
dead instructions and unused functions, contribute slots. Repeated accesses to
the same exact name reuse a slot. Expose the table read-only through the
validated plan, and resolve operations to bounded slot indices before execution.

Whole-file syntax, reference, authority and structural checks remain mandatory.
Every function entry receives ordinary reachable CFG validation. Loads produce
integer kinds and stores consume them, but global initialization is not inferred
from local flow or the presence of a store elsewhere. A load-only name is valid
to admit and can fail explicitly at runtime.

### Keep initialization explicit and state local to one execution

Use a fixed execution-owned table of optional i64 values. Every slot begins
uninitialized. A store marks its slot initialized; a load of an uninitialized
slot returns `global_uninitialized`. Preserve full signed-i64 values without
narrowing. Do not substitute zero or synthesize an initialization store.

Globals share state across all direct and indirect calls in the same execution.
They do not capture lexical bindings or persist across executions. A function
named `__globals_init__` is an ordinary function subject to the same signature,
frame and fuel rules. Run it only when the edition explicitly calls it, and run
each explicit call normally, including repeated calls. The loader performs no
automatic initialization, special ordering or hidden call insertion.

Explicit global operations never change `LOAD_LOCAL`, `STORE_LOCAL`, direct
`CALL` or local indirect-call lookup. A global cannot initialize a local, satisfy
an unresolved call, redirect a builtin or bypass definite local initialization.
Keep the existing registered-local call precedence and target validation.

Global reads and writes derive no service authority. Continue the whole-file
union of actual console operations, including unused functions and dead code.
An uninitialized load after acknowledged output preserves that earlier output;
there is no automatic retry, rollback or fallback.

Clear the entire global table at execution entry and every normal/error return,
alongside frames, operands and existing arenas. This includes exits caused by
invalid caller limits, fuel/frame exhaustion, arithmetic faults, arena limits
or sink refusal. A failed workspace is not resumable and no global state crosses
the synchronous execution API.

### Own exact decimal text in the existing arena

Follow compiler/VM revision `4dad0512dfd3fa4e351f2816196cb48956f5442e`:
integer `CAST text` uses the complete signed decimal value. Produce ASCII digits
with a minus sign only for negative values, no leading plus or extra leading
zeroes, and exactly `0` for zero. The representation is between **one and 20
bytes**, including `-9223372036854775808` and `9223372036854775807`.

This profile admits only the exact target spelling `text` with a proven integer
operand. Existing normalization of i32/i64 operand kinds remains unchanged;
format the actual resulting signed-i64 value. Text inputs, floats, arrays,
records, callable values and aliases such as `string` or `String` remain outside
this conversion. No text-to-integer or floating conversion is added.

Use bounded scratch storage to format the complete value, then preflight its
length against the existing cumulative **65,536-byte decoded-text arena** before
committing any bytes or result. Exhaustion returns the existing `text_limit`.
Each successful conversion owns fresh immutable decoded bytes in that arena;
there is no deduplication or per-frame reclamation. Concatenation and conversion
share the same cumulative bound.

Reuse computed-text handles for stack values, locals, arguments, returns and
text-array elements. Converted digits are already decoded and must not be
interpreted as escaped source text. Existing concatenation, output validation,
per-request/total output caps, truthful sink accounting and complete arena
clearing continue to apply.

All existing instruction, frame, operand, validation, local, function, record,
array and output bounds remain intact. The core stays `no_std`, allocation-free
and unsafe-free. These host structures establish no native storage geometry or
task-service semantics.

## Evidence contract

Preserve all 15 prior proof/oracle sources and rerun all eight prior proof suites
unchanged. Add a separate guarded proof using the actual pinned VM's global
state and integer formatting. The oracle must not delegate those semantics to
candidate code. Reference guard exhaustion or timeout cannot establish
equivalence; finite-reference policy contrasts remain separate from matches.

Compare exact full-i64 results, errors and console bytes for shared globals
across direct/indirect calls, namespace isolation, explicit initialization
ordering and repetition, missing loads, overwrites and extrema. Exercise the
exact slot/name caps, whole-file dead access accounting, stores consuming their
operand, partial output before failure and complete cleanup/reuse after every
exit path.

Test exact decimal zero, signed extrema and intermediate values, computed-text
aliases and returns, text arrays, concatenation, the shared arena boundary and
output limits. Require atomic text-allocation failure and preservation of
earlier acknowledged output. Validation must refuse malformed names, unsupported
global annotations/forms, noninteger stores/conversions, wrong CFG kinds and
unsupported fallback behavior while retaining present-metadata rules and
production main-intent policy.

Run the unmodified upstream runner and complete retained artifacts through
separate raw and production lanes. Preserve all 97 compiler calls, 38 roots,
45 interpreter invocations, auxiliary obligations and the existing telemetry
exit-only exception. Keep all 12 prior raw matches and require an exact match
of the complete `two-books.matrix` artifact before claiming increased coverage.
Refused raw executions remain unmet coverage. Do not inject intent, infer an
initializer, remove unrelated declarations or rewrite instructions to obtain
acceptance. Nearby artifacts retaining unsupported enums or other constructs
remain blocked even if their global instructions become supported.

Require complete repository checks, independent retained-evidence audits,
native artifact continuity and Linux-loader regression before accepting this
checkpoint. Full interpreter qualification and native NIL execution remain
separate closed gates. This host profile changes no kernel, native admission,
task-service mediation or Linux compatibility boundary.

## Qualified checkpoint

All 44 final jobs pass on 711 unchanged non-Markdown source files, with an
independent aggregate audit. The new 105-case proof records 52 exact semantic
matches, seven matching faults, four finite-reference policy contrasts and
42 validation refusals. It retains 63 full-i64 reference comparisons and
prior-profile checks, 105 production checks and one prior-profile corpus
comparison. All nine proof suites pass their combined 756 cases, with 115
separate oracle guards.

Raw corpus coverage reaches 13 of 45 interpreter invocations and eight of 38
roots. The remaining raw refusals are 23 unsupported and nine edition refusals.
All 45 production invocations still refuse: 20 unsupported, nine edition and
16 intent refusals. Five native artifacts remain byte-identical; fresh TCG
regression passes all 8,614 mutation cases. Linux retains 12 requests, 11 returns
and its blocked sequence-12 boundary without exit. Full interpreter qualification
and native execution remain false. The
[Module Globals Log](../../notebook/Development/2026-11-13%20Module%20Globals%20Log.md)
retains exact source/evidence identities, independent audits and remaining gates.
