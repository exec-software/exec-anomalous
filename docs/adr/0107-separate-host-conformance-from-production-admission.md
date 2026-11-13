# ADR 0107: Separate host conformance from production admission

- Status: Accepted — experimental host boundary; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0092, 0093 and 0102–0106
- Scope: Explicit raw scalar validation and host execution, separate from manifest admission.

## Context

The unchanged upstream interpreter corpus tests language semantics. Its 45
interpreter invocations do not carry the authored-main intent required by
production admission. Sending every corpus artifact through production
packaging conflates an unmet application policy with interpreter behavior.
Adding more instructions alone cannot resolve that distinction.

Do not rewrite fixtures, inject intent, strip helpers, change the upstream
runner or relax the accepted production admission contract to obtain a pass.
Compiler conformance, interpreter semantics, production manifest admission and
native service mediation remain separate obligations.

## Decision

Add an opt-in `host_conformance` feature, disabled by default, to the flow
validator and executor. Reject this feature at compile time on freestanding
`target_os = "none"` builds. The feature is for bounded host experiments, not
an alternate native loader or a runtime policy switch.

Expose `conformance::validate_scalar` over a held raw edition, an explicit
known service ceiling and caller-owned validation storage. Return a distinct
private-field `checked_edition` borrowing the exact bytes and immutable plan.
It records their SHA-256 identity, optional authored intent and derived/effective
service surface. There is no conversion to `admitted_program`, no package
encoder, no public raw-plan executor and no conformance wire-profile tag.
Existing profiles 1, 2 and 3 and their envelopes keep their accepted meanings.

Share the profile-3 parser, whole-artifact reference and authority scan, CFG
validation and all structural limits through a private validation purpose.
Production encoding and admission always select the purpose requiring authored
main intent. Raw conformance permits only its **absence**. An annotation that is
present must still satisfy the same supported shape, placement, uniqueness,
length and text rules. Missing intent is represented as absence, never inferred
from a filename, description or generated text. This checkpoint does not broaden
annotation semantics or support unknown metadata.

Both purposes still require one supported zero-argument integer-returning main,
complete valid syntax and resolved references, type/stack/local checks, and a
whole-artifact authority union including unused functions and dead instructions.
The trusted host supplies the ceiling; a wider ceiling cannot add authority not
derived from the edition. Unknown effects and unknown ceiling bits still refuse.
Neither digest, optional intent nor a checked witness is a kernel grant.

Add a separate `anomalous_program_executor::conformance::execute` taking only
the raw checked witness. It shares private iterative dispatch, fixed storage,
limits, storage clearing and console mediation with the existing production
entry. Keep the public production `flow::execute` signature restricted to
`admitted_program`. The shared implementation remains private to the crate.
Host sinks receive the identity of the exact edition and no ambient authority.
Earlier acknowledged output survives later faults; refusal causes no retry.

## Evidence contract

The new proof records two independent lanes over identical retained corpus
bytes: raw validation/execution and production profile-3 admission/execution.
Preserve all 97 compiler calls, 38 root fixtures and 45 interpreter invocations,
the upstream schema/diagnostic/import obligations and its existing single
clock-dependent exit-only exception. Record actual candidate results and output
against the reference; raw validation refusal is unmet semantic coverage, not a
pass. Production refusal is evidence of application policy, not proof of raw
semantics. Neither lane may substitute for the other.

Use separate authored boundary fixtures to prove intentless execution, exact
result/output identity, retained production refusal, optional valid intent,
invalid annotations, whole-file effect accounting, ceilings and runtime faults.
Compile-fail tests must preserve witness privacy, borrow isolation and the
production entry's type boundary. Verify the host feature is excluded from
freestanding builds. Preserve original profile-1, profile-2 and profile-3 proofs
and their pinned reference sources unchanged; execute all required repository
checks. Independently inspect retained process records and exact input/tool
identities before recording a qualified checkpoint.

## Qualified checkpoint

All 29 final jobs pass on 678 unchanged source files. The new proof passes
27 boundary cases and preserves four raw corpus matches while independently
retaining all 45 production refusals. The original 126-, 77- and 61-case proofs
pass unchanged. Both freestanding feature builds refuse with the explicit guard;
independent process/artifact audits and all required regressions pass. The
native admission artifacts remain byte-identical, and Linux retains its prior
loader boundary. The [Host Conformance Boundary Log](../../notebook/Development/2026-11-13%20Host%20Conformance%20Boundary%20Log.md)
records exact scope, failures, tool/source identities and receipts.

## Remaining gates

This distinction enables honest measurement of language support; it does not
declare the interpreter corpus qualified. Unsupported language, binary formats,
large editions, metadata and service families remain separate work. Native
execution still requires full corpus qualification, a measured interpreter
artifact, production-admitted dispatch, task-bound effects and complete cleanup.
The native admission-only loader and Linux compatibility boundary do not change.
