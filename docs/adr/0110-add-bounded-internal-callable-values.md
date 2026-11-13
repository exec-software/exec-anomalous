# ADR 0110: Add bounded internal callable values

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0102 and 0105–0109
- Scope: Host-only profile 6 for typed noncapturing internal function references and local indirect calls.

## Context

[ADR 0109](0109-add-bounded-immutable-typed-arrays.md) supplies the array
operations used by the pinned compiler's paragraph examples. Their remaining
function-value forms are `PUSH_FN_PTR` and `CALL` through ordinary or generated
temporary locals. Noncapturing anonymous functions lower to ordinary internal
declarations and references to those declarations.

The pinned VM represents a function reference as a string. At a `CALL`, a
current-frame string local can redirect dispatch before a direct declaration or
builtin is considered. Accepting a new producer while retaining the old static
call resolver would therefore be incorrect. Admitting arbitrary string-derived
targets would also lose the validator's established target and type guarantees.

Add a separate profile with opaque callable provenance, exact signature
identities and explicit local precedence. Preserve earlier profiles and their
collision refusals. Captures, callable parameters and results, external
references and general dynamic dispatch remain outside this slice.

## Decision

### Preserve explicit profile and witness boundaries

Add profile **6 in both existing envelope headers** through `callable::encode`
and `callable::admit` in `anomalous-program-flow`. Preserve version 1, canonical
extents, reserved zeros, hashing, immutable validated plans and production main
intent. Earlier entry points retain their grammar, call resolution and profile
refusals. Profile 6 rejects every other tag; no parser fallback is added.

Add `conformance::validate_callable` under the existing opt-in host-only feature.
Its distinct raw witness records profile 6 and exact edition identity. Raw
conformance permits absent main intent but preserves every rule for metadata
that is present. It cannot emit a package or produce a production witness.
Both purposes retain explicit service ceilings, whole-file syntax/reference
checks and reachable CFG validation from every declared function entry.

### Admit only references to supported internal declarations

Profile 6 inherits profile 5 and adds exact `PUSH_FN_PTR <name>`. The name must
be a supported simple identifier resolving to a declared internal function in
the complete edition. Resolve forward references and references in dead code.
Unknown names, builtin references, external targets and additional capture
operands refuse even when execution would not reach them.

Represent the validated operation as `function(target)`. The runtime value is
an opaque private handle containing the resolved target and its signature
identity. Text with the same spelling cannot construct that value. Locals and
operand slots may copy or discard it; casts, arithmetic, comparisons, conditions,
text concatenation, array operations and console output cannot consume it as a
scalar or array. No raw address or external entry point is exposed.

Targets retain the existing admitted parameter and result types: `i32`, `i64`,
`text`, `array<i64>` and `array<text>`, with `void` results also supported.
Callable parameter/result syntax and nested callable signatures remain
unsupported. `main` retains its zero-argument integer-returning contract.
Every target starts with its own validated parameter stack and empty locals;
references do not capture or inherit caller bindings.

### Track exact signatures without expanding CFG storage unboundedly

Assign each declared function a compact signature identity: the index of the
first declaration with exactly the same parameter-type sequence and result
type. Parameter names do not affect identity. Distinguish `i32` from `i64`,
including their result types, even though existing scalar operand validation
normalizes both to integer values and performs no implicit boundary cast.

Use a private 31-bit key for the existing bounded signature vocabulary: eight
three-bit parameter codes, one three-bit result code and a four-bit arity.
The existing maximum of 64 functions fits the canonical index in `u8`.
Expose the signature only as a read-only function property and represent its
abstract kind as `function(u8)`. The tag remains compact in the fixed CFG
states; this adds no unbounded target-set allocation.

Different targets with the same exact signature may merge on the operand stack
or in a definitely initialized local, including loop backedges and reassignment.
Different signatures refuse those merges and stores. Calls still verify the
complete argument count and operand kinds in declaration order and produce only
the declared result kind. Ordinary text cannot merge with a callable.

### Resolve registered locals before direct functions or builtins

Profile-6 `CALL <name> <count>` accepts the existing simple and generated-local
name grammar. Counts retain complete unsigned decimal spelling, with leading
zeros permitted, and the existing maximum of eight arguments. Defer call
classification until the complete function has been parsed.

If `LOAD_LOCAL` or `STORE_LOCAL` registers the called name in the owning
function, resolve it to `call_local { local, arguments }` before considering a
direct declaration or builtin. A later or dead store still registers that local.
Every reachable local call requires it to be definitely initialized with a
callable of the required signature. A text, integer, array or uninitialized local
refuses; do not silently fall back to a same-named direct function or builtin.
This is deliberately narrower than the pinned VM's non-string-local fallback.

If there is no registered local, resolve a declared function with its exact
arity, or one of the already supported console builtins with its required
arity. Unknown targets refuse, including in dead code. A dead registered-local
call receives lexical, local-reference and count-bound checks without invented
operand state or a speculative target signature, following ADR 0105.

Profile 6 permits local names to shadow declarations and console builtins under
these rules. Function declarations named builtins remain unsupported. Parameter
names retain their existing builtin and function-name collision restrictions;
a bare parameter declaration does not create a preinitialized local slot.
Earlier profiles keep all previous local collision refusals.

### Preserve complete authority accounting and bounded execution

Derive console authority from actual resolved console operations and explicit
console instructions across every function, including unused functions and dead
instructions. A local callable named `println` does not itself derive console
authority when it resolves to an internal call. Any console operation in its
target, or another unused function, remains part of the complete artifact union.
No function value or signature grants additional authority.

Indirect dispatch reads the validated current-frame local, verifies its target,
signature and arity, and enters the existing iterative call machinery. Preserve
caller stack prefixes, argument order, isolated locals and declared result
kinds. Aliases retain their target when another local is rebound. No closure
environment, heap allocation, callable arena or new service family is added.

Retain existing frame, operand, instruction, validation-transfer, local,
function, array, computed-text and console limits. Recursive indirect calls
consume the same bounded frames and fuel as direct calls. Clear callable values
with all operand and local storage at execution entry and every normal/error
return. No handle escapes synchronous execution. Previously acknowledged sink
effects remain visible after later failure, without retry or rollback.

## Evidence contract

Preserve every earlier profile, proof suite and reference source unchanged.
Use the actual pinned compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e` in a separate guarded callable proof.
Record its string representation and lexical-call precedence as reference
semantics while keeping the candidate's narrower typed admission explicit.
Reference timeout or guard exhaustion cannot establish equivalence.

Compare exact full-i64 results, errors and console bytes for named references,
noncapturing lambda declarations, local aliases and generated callee temporaries,
same-signature branch selection and rebinding, direct/builtin shadowing,
text/array arguments and returns, caller-prefix preservation and recursion.
Exercise ordinary and raw witnesses separately. Include frame/fuel exhaustion,
later faults, truthful sink accounting and complete storage clearing.

Require validation refusals for arbitrary text targets, absent or partially
initialized locals, different exact signatures, wrong argument counts and kinds,
unknown or builtin references, capture operands, callable signature declarations,
scalar/array/console misuse, unresolved dead references and earlier-profile
retagging. Verify exact i32/i64 distinctions, compact kind storage and the
unchanged whole-file authority union. Test a shadowed builtin with no console
body at ceiling zero and an unused console target requiring console authority.

Run the unmodified upstream runner and all complete retained artifacts through
separate raw and production lanes. Preserve the 97 compiler calls, 38 roots,
45 interpreter invocations, auxiliary obligations and the single existing
telemetry exit-only exception. Refused raw executions remain unmet semantic
coverage. Do not inject intent, rewrite instructions or strip declarations to
obtain a match.

Require complete repository checks, independent retained-evidence audit, native
artifact continuity and Linux-loader regression before accepting this checkpoint.
Full interpreter qualification and native NIL execution remain separate closed
gates; no kernel, native admission, service-mediation or Linux boundary changes
are authorized by this host profile.

## Qualified checkpoint

All 38 final jobs pass on 698 unchanged non-Markdown source files. The new
103-case proof records 40 semantic matches, two matching faults, seven finite
policy contrasts and 54 validation refusals. Nine of 45 unchanged raw corpus
invocations match, including six of 38 roots; all 45 retain production refusal.
All six preceding proof suites pass unchanged. Independent retained-evidence
audits verify exact inputs, full-i64 results, output bytes, corpus process records,
30 oracle guards, original-checkout preservation and native artifact continuity.
The required regression matrix passes, including the unchanged Linux-loader
boundary. The [Internal Callable Values Log](../../notebook/Development/2026-11-13%20Internal%20Callable%20Values%20Log.md)
records exact evidence and remaining gates.
