# ADR 0117: Admit canonical compiler local identities

- Status: Accepted — experimental host profile
- Date: 2026-11-21
- Builds on: ADRs 0102 and 0105–0116
- Scope: Experimental host profile 13

## Context

The pinned NIL compiler gives a repeated source binding a distinct storage
name. The complete 7,966-byte `records-and-choices` edition uses this spelling
for the second pattern binding named `code`. Profile 12 rejects the edition
during whole-function validation.

## Decision

Select profile 13 explicitly with `hygienic_locals::encode` and
`hygienic_locals::admit`. Record tag 13 in both existing envelope headers.
The opt-in `conformance::validate_hygienic_locals` entry point keeps the
existing separate raw validation witness. Production still requires authored
main intent, exact byte identity, and an explicit service ceiling.

Accept `$ny.local.<counter>.<name>` in local storage operands and existing
local indirect call resolution. The counter is either `0` or an ASCII decimal
string beginning with a nonzero digit. The name is a simple ASCII identifier.
The complete spelling fits within the
existing 128-byte bound. Malformed spellings return the syntax error and
overlong spellings return the limit error.

Use the full spelling as an opaque identity within the current function.
Every distinct spelling consumes one of its existing 128 local slots.
Repeated uses of that identity share a slot. A function's names and counters
are independent of other functions. The decimal counter has no numeric
meaning in admission or execution. Ordinary local names and existing
`$ny.temp` identities retain their grammar. Only local operands use the
new grammar. Function, parameter, global, field, and label declarations
continue to use their prior identifier rules.

Resolve local calls by exact name to the existing callable slot and retain
its definite initialization, signature, and argument checks. A source-name
suffix carries no lookup or authority of its own. Ordinary opcode dispatch
continues to use resolved slot indices and the existing frame boundaries.
Debug records continue to carry the source spelling and the independent
storage spelling as descriptive metadata.

## Evidence

Compare the complete invocation 35 to the pinned VM using the identical
edition bytes and full signed result, console bytes, and metadata. This
edition places the generated local in the second `Failed` branch, while its
main path selects the first branch. Exercise storage and indirect calls
through generated slots in separate executed fixtures. Check canonical
spelling, identity isolation, control flow, resource bounds, prior profile
refusal, and both admission paths.

All 48 qualification jobs passed on the first attempt. Fourteen language
suites cover 1,430 boundary cases and the ten reference guard jobs verify
535 checks. The complete retained replay matches 19 of 45 raw invocations
and 13 of 38 roots. All 45 production admission attempts refuse. The
[Hygienic Locals Log](../../notebook/Development/2026-11-21%20Hygienic%20Locals%20Log.md)
records the test and artifact identities. Full corpus qualification and
native NIL execution remain separate gates.
