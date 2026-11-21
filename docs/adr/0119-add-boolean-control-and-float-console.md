# ADR 0119: Add boolean control and floating-point console output

- Status: Accepted — experimental host profile
- Date: 2026-11-21
- Builds on: ADRs 0115–0118
- Scope: Host profile 15 for boolean branches, short-circuit local joins, and finite binary64 console output.

## Context

The complete `bindings.matrix` edition uses boolean values with conditional
jumps and `NOT`. Its generated short-circuit OR stores an integer on one
path and a boolean on the other, then reads the joined local and explicitly
casts it to `bool`. The pinned VM represents both kinds using signed
integers. The host validator preserves distinct kinds to check function
signatures, arithmetic, and aggregate access. The edition also prints a
finite binary64 value through the built-in `println` call.

## Decision

Explicitly select profile 15 through `boolean_control::encode` and
`boolean_control::admit`, with tag 15 in both envelope headers. The opt-in
`conformance::validate_boolean_control` supplies a separate raw witness.
Production admission requires authored intent, exact edition identity, and
an explicit service ceiling.

- `JMP_IF`, `JMP_IF_NOT`, and `NOT` accept an integer or a boolean.
  Integers use nonzero truth, and `NOT` returns integer zero or one.
- A control-flow join can merge integer and boolean values in an initialized
  local. A load from a mixed local must be immediately followed by `CAST
  bool`. The existing checked cast accepts only zero and one. A subsequent
  store establishes the exact kind of the new value. Operand stacks and
  callable signatures continue to require exact kinds.
- The built-in console `CALL` forms accept finite binary64 values in
  addition to integers and text. Local call resolution takes precedence over
  the console fallback. Built-in names remain reserved for function
  declarations. Intrinsic interop and explicit console opcodes retain their
  integer and text contract.

Float output uses classic general notation with six significant digits.
The fixed-capacity formatter rounds to nearest, ties to even, and uses
fixed notation for rounded exponents from −4 through 5. Scientific notation
includes an explicit exponent sign and at least two exponent digits. Both
forms trim fractional trailing zeros and preserve the sign of negative zero.
Float output uses the existing authority, payload, byte budget, and
acknowledgment path. Each nonempty print makes one request.

## Evidence

Compare the complete retained `bindings.matrix` artifact with the pinned VM
using the full signed result, console bytes, and metadata. Exercise both
boolean branch outcomes, mixed-local joins, exact kind refusals, and
floating-point rounding and output boundaries. Replay all retained corpus
invocations, earlier profiles, and native and Linux-loader regression checks.

All 52 qualification jobs passed on their first attempt. Sixteen language
suites covered 1,632 boundary cases. The unchanged `bindings.matrix` edition
returned zero with 24 matching console bytes. Raw coverage reached 21 of 45
invocations and 15 of 38 roots. All 45 production admission attempts refused.
