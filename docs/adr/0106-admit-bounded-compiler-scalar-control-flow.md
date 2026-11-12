# ADR 0106: Admit bounded compiler scalar control flow

- Status: Accepted — bounded host checkpoint; full corpus and native execution remain gated
- Date: 2026-11-12
- Builds on: ADRs 0102, 0104 and 0105
- Scope: Host-only profile 3 for compiler-generated scalar locals and range loops.

## Context

[ADR 0105](0105-validate-bounded-function-control-flow-before-host-execution.md)
qualifies calls and control flow, but its profile 2 deliberately refuses dollar
temporaries and operand-less arithmetic. The pinned compiler emits both for
ordinary range loops and short-circuit expressions. Changing those refusals
silently would change an accepted derivation profile's meaning.

## Decision

Add explicit profile **3 in both existing envelope headers**, with separate
`scalar::encode` and `scalar::admit` entry points in `anomalous-program-flow`.
Keep envelope version 1, its canonical layout, hashing, authored-main intent and
whole-artifact authority checks. Existing profile-1 and profile-2 entry points
keep their contracts and reject profile-3 packages. The new entry point rejects
either older tag. Never choose a parser by trying alternatives until one accepts.

Share the parser, resolver, CFG validator and immutable admission witness with
profile 2 through a private, explicitly selected policy. Expose the admitted
profile as a read-only witness property. A caller cannot construct or replace
the validated plan. Neither profile selection nor a temporary's spelling grants
authority or bypasses initialization, type, stack or target checks.

Profile 3 adds exactly two forms:

- `LOAD_LOCAL` and `STORE_LOCAL` may name
  `$ny.temp.<simple_ascii_identifier>.<canonical_decimal>`. The full name is at
  most 128 bytes, the decimal suffix is `0` or starts with `1` through `9`, and
  all remaining suffix bytes are decimal digits. Names are compared exactly;
  the suffix is not parsed into a machine counter or normalized. These locals
  share the existing 128-slot per-function/frame limit with ordinary locals.
- Operand-less `ADD` normalizes to the existing integer `ADD i64` operation.
  The CFG must establish two integer operands on every reachable path. It has
  wrapping 64-bit semantics, with no implicit narrowing to 32 bits.

Dollar names remain forbidden for function names, parameters, constants, labels
and call/branch targets. Dotted compiler prefixes such as `query.source` remain
outside this bounded scalar subset. Supporting some generated temporary names
does not claim support for every temporary emitted by the compiler.

The reference VM also uses operand-less `ADD` for noninteger values. This
profile admits no floating-point or array producers, and a reachable text
operand fails CFG validation. All other untyped arithmetic remains unsupported.
Dead instructions retain complete syntax/reference/authority checking without
invented operand states, exactly as in ADR 0105.

Use the existing iterative executor and caller-owned storage. The new forms
normalize into its existing typed operations; no new dispatch path, allocator,
service family, native vessel or kernel feature is introduced. All validation,
frame, stack, instruction and console limits remain those of ADR 0105.

The pinned compiler evaluates range bounds once in start-then-end order and
compares its hidden 64-bit index against an exclusive endpoint. It explicitly
casts the visible loop variable to 32 bits. Preserve those emitted instructions,
including casts, continue increments and break targets. Do not infer execution
semantics from the source-level loop syntax or rewrite complete compiler output.

## Acceptance contract

Retain complete `.ny` sources and byte-identical `.nil` outputs from qualified
compiler revision `4dad0512dfd3fa4e351f2816196cb48956f5442e`, using the existing
explicit context-registration suppression option. Independently compare exact
results and output for empty and nested ranges, bound evaluation, break/continue,
short-circuit side effects, scratch-local joins and visible-index narrowing.
Test integer wrap, name/context refusals, shared limits, definite initialization,
profile isolation and resource exhaustion. A finite reference completion versus
a candidate resource refusal is a policy contrast, never a semantic match.

Run the original 126-case and 77-case proofs unchanged. Retain a separate new
proof, independent verification, source/tool identities, unsuccessful attempts,
all required repository checks and native/Linux regression evidence. A new
reference oracle may admit the new forms; leave the older oracle sources intact.

All 27 final jobs passed on 672 unchanged non-Markdown source files. The new
61-case proof records 27 exact semantic matches, one finite-reference fuel-policy
contrast and 33 admission refusals, plus 28 profile-2 raw-edition refusals.
The original 126-case and 77-case proofs pass unchanged. Independent verifiers
check raw outcomes, complete corpus captures and tool/input identities. The
native admission artifacts remain byte-identical, and fresh native delivery and
Linux loader regressions preserve their existing behavior. The full receipt is
in the [Compiler Scalar Flow Log](../../notebook/Development/2026-11-12%20Compiler%20Scalar%20Flow%20Log.md).

## Remaining gates

The unchanged upstream corpus remains a full interpreter gate. Preserve all
97 compiler calls, 38 root cases and 45 interpreter invocations, counting
unsupported or refused candidate executions as unmet coverage. Raw semantic
conformance and production manifest admission require a later explicit
architectural distinction: do not inject intent into corpus artifacts or waive
production admission policy. Full interpreter qualification and native NIL
execution remain false until their separate obligations are met.
