# Bounded immutable typed arrays — 2026-11-13

This checkpoint extends the host interpreter with explicit profile 5 under
[ADR 0109](../../docs/adr/0109-add-bounded-immutable-typed-arrays.md). It adds
homogeneous `i64` and `text` array literals, lengths and indexed reads, including
canonical typed helper parameters and returns. The unchanged `control-flow.matrix`
edition is the next complete corpus artifact requiring this subset. No kernel,
native service, desktop or Linux-loader behavior is added here.

## Ownership and implementation

The existing iterative machine now owns fixed pools of 128 array descriptors and
1,024 scalar elements alongside its 65,536-byte computed-text arena. One literal
contains at most 256 elements. Private typed handles survive helper returns and
preserve aliases. Literal text stays borrowed, computed text keeps its decoded
span, and array reads introduce no conversion or repeated decoding. Empty arrays
consume a descriptor, and no storage is reclaimed until the run ends.

Allocation checks every operand kind, the caller's stack floor, replacement
stack space and both capacities before publishing. `array_limit` identifies
exhaustion; previously acknowledged console effects survive. Entry and every
normal/error return scrub descriptors, elements, text, frames and values. The
validator proves array kinds through joins and signatures and explicitly refuses
array console output and scalar operations on handles.

Pinned reference revision `4dad0512dfd3fa4e351f2816196cb48956f5442e` uses
`ArrayHandle` for literals and preserves source order. Default bounds mode returns
integer zero or empty text for negative, empty and too-large reads. The candidate
selects that behavior explicitly. The independent C++ host disables the ambient
strict-bounds override and refuses raw integer pointer aliases, mutation and
uncounted overloaded array addition. Its larger resource caps are safety limits,
never semantic equivalence credit.

## Development findings retained

Artifacts live outside Git under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-array-2026-11-13/`.
The initial `development/` logs retain three Rust test-only lifetime errors:
per-iteration temporary editions were paired with a longer-lived validation
workspace. Fresh workspaces fix the tests without changing validation behavior.

The first differential run matched all eight expected raw corpus invocations but
failed two expected-error assumptions. Unknown `CALL print` is refused during
reference resolution before reachable operand checking, so its correct error is
`manifest_unsupported`. Production invocation 31 (`paragraphs.matrix`) now parses
its `array<i64>` helper and reaches missing main intent before later unsupported
operations. Its first error changes from unsupported to intent; it still refuses
production, and raw execution remains unsupported. Production totals therefore
become 24 unsupported, nine edition and 12 intent refusals.

Independent source review also found that the new oracle's count guard required
canonical decimals while the candidate and pinned VM allowed leading zeros.
The oracle now accepts complete unsigned decimal tokens and recognizes any zero
spelling in its stack-capacity preflight. New `000` and `0001` positive cases and
admission tests cover the distinction. Earlier oracle sources are untouched.

The `development-final/` rerun preserves fresh evidence after those corrections.
Final qualification and independent audit evidence are recorded below; no
failed preliminary result is overwritten.

## Independent language evidence

All five preceding proof suites pass unchanged: 126 profile-1, 77 control-flow,
61 scalar-flow, 27 raw-conformance and 55 computed-text cases. The new proof
retains 87 boundary editions: 36 exact semantic matches, one matching runtime
fault, seven finite resource-policy contrasts and 43 validation refusals. There
are 44 full-i64 C++ VM comparisons, 87 production counterparts, 44 prior-profile
raw/production pairs and one new corpus prior-profile pair. Refusals and resource
contrasts are not counted as semantic matches.

The unchanged corpus runner retains 97 compiler calls, 38 roots and 45
interpreter invocations, plus all earlier auxiliary obligations and the single
existing telemetry exit-only exception. Eight raw invocations match, including
five roots; the 37 refusals are 28 unsupported and nine edition errors. All 45
production attempts refuse independently. The newly matching 7,821-byte
`control-flow.matrix` edition has SHA-256
`2e4afc09a9d4af086664764b4d171d1544a81cb670d66a273472254dddc5a0d5`;
it returns zero with 54 exact console bytes. No helpers, metadata or expectations
were removed or injected.

The final proof summary hash is
`150c4788ae95fa80a5e0d865ae0dc8e8824434b5082e87ba96f42d32572764b7`.
Independent retained-evidence audit hash:
`5eb52fc92348a18ef797de72a2b009742f3184d795edd8e1aa8fe6704e510d25`.
The raw candidate hash is
`e4c98a54f087d4b291f85d13255ef44250a5ec1f3dc21bbef4527c36d4825974`;
the production candidate hash is
`76a2a7c1b1436379e6c2549771bf5424c1d8560144bb18622962607822c620ee`;
the array C++ oracle hash is
`f5f1c083f7f3f1a5767bc47cb7bd3db487064c396643ac1d420f5fe19e9fc211`.
Independent checks read exact retained inputs, process records, output bytes and
tool/source identities without importing the new proof harness.

The preserved text-oracle guard and ten new array-oracle guards pass. New checks
exercise descriptor/element/stored-text bounds, integer-handle rejection,
unaccounted array addition, mixed/nested literal rejection, default bounds under
a strict environment override and computed-text limits. These have no candidate
semantic-coverage credit. The inherited oracle header still has one obsolete
sentence saying the guard permits neither array nor float producers; the
explicit profile-5 paragraph and implemented producer/operation checks define
the actual subset. A comment-only cleanup is deferred to the next checkpoint
to preserve this source freeze.

## Next bounded gate

A read-only audit of all 45 exact retained editions identifies
`paragraphs.matrix` (invocation 31) as a focused next option: typed, noncapturing
internal function references and indirect calls through locals. Its complete
7,278-byte edition still needs `PUSH_FN_PTR` and indirect `CALL`; success would
need a fresh full proof and could move raw coverage to nine of 45. A separate
record slice could cover three smaller editions, but requires shared mutable
record identity, declared field layouts and record metadata. Neither projection
is implemented or counted here. The external `next-slice-audit.json` records the
exact inputs and gates with SHA-256
`6509f49d6c2845d60a435479d2c72b568e5b09fba0c8d50f451ca8baabd091c9`.

## Frozen validation scope

Final qualification starts from `b3e077820bcf5652f65790e808a610371efdff0e`
on `codex/nil-typed-arrays`. Its 692 non-Markdown source files are pinned by
`qualification/source-sha256.json`, SHA-256
`02aac003806e5cc6833c75d1c4ee4e2ce64f3baa3c2f574857e840070beb76a4`.
Implementation, probes, proof sources and oracle sources remain unchanged during
the final run; only documentation and external audit files are completed later.

The final `make check` records 66 default / 82 conformance flow tests, three
compile-fail witness checks, 52 default / 58 conformance executor tests and one
compile-fail dispatch check. The broader suite retains 648 default / 731
all-feature kernel tests, the intervening feature combinations, 425 tooling
tests, 37 native-runtime tests and 18 manifest tests. Python proof selftests
remain separate: six baseline, five flow, four scalar, five conformance, five
text and six array checks.

Fresh native manifest boots at 96 and 320 MiB retain four admissions, six
refusals, 12 reads, one fixed loader marker and zero executed NIL editions.
Independent transcript verification rejects all 8,614 hostile mutations.
The boot image, kernel, client, flat client bytes and wrapper are byte-identical
to the preceding checkpoint. This is fresh TCG evidence; historical AMD KVM
coverage is inherited only under exact artifact/source identity. No fresh KVM
or VirtualBox run is claimed.

## Accepted qualification

All **35 final jobs passed on their first attempt**, with 1,486.1 seconds of
recorded serial execution. This includes all six proof suites, both oracle guard
jobs, host-only feature exclusion, native manifest admission, `make check`, all
required boot/smoke targets, normal exclusion, manifest proof, native delivery
and Linux loader entry. `make check` took 738.1 seconds. Independent verification
passed the complete retained evidence and exact command inventory.

`qualification/summary.json` has SHA-256
`4e302aa29a40cafc419a0ad72dc9046d3b76e6964603ab5ba8e4b99007ffe0eb`.
The native delivery boot image remains
`00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b`.
Both Linux runs retain 12 requests, 11 returns and unanswered sequence 12:
`mmap(0x100028000,0x189000,5,0x812,3,0x28000)`. No Linux exit or new compatibility
behavior is claimed.

The original checkout's tracked diff remains
`28d1d1f342846a1cbf8e637b0de1e6eedf71cfccd600e0825b347edce77408c3`,
and all seven unrelated files remain unchanged. The isolated worktree checkpoint
is committed and pushed separately. Full corpus qualification and native NIL
execution remain false; the new evidence qualifies only this bounded host slice.
