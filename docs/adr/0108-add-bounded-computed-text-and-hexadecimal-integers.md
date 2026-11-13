# ADR 0108: Add bounded computed text and hexadecimal integers

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-13
- Builds on: ADRs 0105–0107
- Scope: Explicit profile 4, text/text concatenation and signed-i64 hexadecimal literals.

## Context

The raw conformance path now measures language behavior independently of
production intent policy. Three small, unchanged corpus editions need computed
text: `short-extension.mat` returns a concatenated string from a helper, and the
scalar Ny/Matrix twins also use hexadecimal constants. Returning a borrowed
frame buffer or concatenating escaped source spelling would give incorrect
lifetime or decoding semantics.

## Decision

Add explicitly selected profile **4 in both existing envelope headers** through
`text::encode` and `text::admit`. Keep envelope version 1 and its layout. Existing
profiles 1–3 keep their grammar and refuse profile-4 packages; profile 4 refuses
older tags. No parser fallback or automatic profile selection is added.

Add `conformance::validate_text` under the existing opt-in, host-only feature.
It returns the distinct raw checked witness, now with a read-only selected
profile property. `validate_scalar` still selects profile 3. Neither raw path
produces a package or an admitted witness. Production requires authored main
intent; raw conformance permits only its absence. All rules for present metadata,
whole-file authority, explicit ceilings and immutable exact bytes remain intact.

Profile 4 inherits profile 3 and adds only:

- Exact operand-less `STR_CONCAT`, consuming two text values and producing one
  text value. Every reachable incoming flow must prove both operand kinds.
  Integer/float conversion, arrays and other overloaded addition remain outside
  this profile. Dead instructions retain syntax/reference/authority checks
  without invented operand states.
- `PUSH_CONST` hexadecimal integers: optional ASCII `+` or `-`, then `0x` or
  `0X`, followed by one or more ASCII hexadecimal digits. Leading zeroes are
  allowed. Require complete consumption and signed-i64 range, including negative
  minimum; positive `0x8000000000000000` refuses. Whitespace inside the token,
  separators, missing digits, suffixes and overflowing magnitudes refuse.
  Existing decimal grammar is unchanged.

These forms follow the pinned compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`: hexadecimal recognition precedes
floating-point detection, conversion is signed and consumes the complete token,
and concatenation pops right then left and forms decoded left plus right.

## Own computed bytes across calls

Keep literal values as borrowed escaped source text. Add a separate computed
text value carrying private offset/length handles into an execution-owned,
fixed **65,536-byte decoded-text arena**. Locals, stack values, arguments and
returns copy these handles. Frame teardown never reclaims their bytes, so text
returned from a helper and aliases retained by its caller stay valid.

Decode each literal operand exactly once when concatenating. Computed operands
are copied as bytes and never decoded again. Reuse the existing literal decoder,
including its preservation of the reference loader's `\b` spelling. This adds
no escape syntax. Unicode bytes, literal backslashes, quotes and escape sequences
formed across a concatenation boundary must retain their reference meaning.

Preflight both complete decoded lengths and cumulative arena capacity before
writing. Concatenation creates a fresh immutable result; it never overwrites an
operand or aliases pending storage. Every nonempty result consumes its complete
length, including concatenation with an empty operand. An empty result consumes
zero bytes and remains valid at full capacity. No garbage collection, deduplication,
per-frame reclamation or reuse during a run is introduced.

Exhaustion produces `text_limit`, with no result publication or sink call for
that instruction. Earlier acknowledged console output remains visible. Clear
every arena byte, used count, frame and value at entry and every normal/error
return. Values cannot escape the synchronous execution API. These host storage
requirements establish no native stack/mapping geometry.

Reuse the iterative executor and existing instruction/frame/value budgets. The
arena bound is fixed, distinct from the output bound. Computed console output
uses the same identity/authority checks, complete UTF-8/control validation,
4,096-byte per-request payload cap, 65,536-byte total-output cap and truthful
attempt/acknowledgement accounting as literals. Empty non-newline output remains
a no-op. Oversized or disallowed output refuses without truncation or splitting.
There is no retry after a service refusal.

## Evidence contract

Preserve all four earlier proofs and reference sources unchanged. Add a separate
guarded oracle using the actual pinned C++ VM, with text/text runtime checks and
explicit larger reference resource caps. A reference guard failure or timeout
cannot establish equivalence. Exercise its exponential-concatenation cap as an
oracle safety check with no semantic-coverage credit.

Compare exact full-i64 results, errors and console bytes for signed hexadecimal
boundaries, empty/Unicode/escaped text, repeated concatenation, aliasing, helper
arguments/returns and later faults. Test old-profile refusal, malformed syntax,
CFG mixed-kind refusal, exact arena exhaustion, sink/output policy and storage
clearing. Keep finite-reference resource contrasts distinct from semantic matches.

Run the original upstream runner and all complete, unmodified artifacts through
separate raw and production lanes. Preserve 97 compiler calls, 38 root cases,
45 interpreter invocations, all auxiliary obligations and the existing single
telemetry exit-only exception. Count refusals as unmet semantic coverage and
retain exact known positive artifacts as a regression contract. Do not inject
intent, strip helpers or alter expectations to obtain production admission.

Require the repository's complete checks, independent retained-evidence audit,
native artifact continuity and Linux-loader regression before accepting the
checkpoint. Native NIL execution and full interpreter qualification remain
separate, closed gates.

## Qualified checkpoint

All 32 final jobs pass on 685 unchanged non-Markdown source files. The new
55-case proof records 34 exact semantic matches, one matching fault, six finite
policy contrasts and 14 validation refusals, with independent production and
prior-profile checks. Raw corpus coverage reaches seven of 45 invocations,
including four roots; all 45 retain production refusal. The four earlier proofs
pass unchanged. Independent artifact/process audits, the reference resource
guard and all required regressions pass. Native admission artifacts remain
byte-identical and Linux retains its prior loader boundary. The
[Computed Text Log](../../notebook/Development/2026-11-13%20Computed%20Text%20Log.md)
retains exact evidence, failed preliminary expectations and remaining gates.
