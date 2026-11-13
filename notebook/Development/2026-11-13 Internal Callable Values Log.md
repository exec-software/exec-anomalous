# Bounded internal callable values — 2026-11-13

This checkpoint extends the host interpreter with explicit profile 6 under
[ADR 0110](../../docs/adr/0110-add-bounded-internal-callable-values.md). It adds
typed references to declared internal functions and calls through registered
locals, including the compiler's generated callee temporaries. The target is the
complete unchanged `paragraphs.matrix` edition. This slice adds no kernel,
native service, desktop or Linux-loader behavior.

## Implementation and reference boundary

A private runtime handle carries a function-table target and canonical signature
identity. It is not an address or a string. Exact declared parameter/result types
define the signature; `i32` and `i64` remain distinct for target joins even though
the established argument-kind check accepts integer operands for both. A compact
31-bit signature key and `u8` canonical representative keep abstract kinds at two
bytes in the bounded validation workspace.

Aliases retain their targets when another local is rebound. Different targets
with the same signature can merge at branches or loop backedges. Registered
locals take precedence over same-named functions or console builtins, including
names registered by a later or dead store. A reachable local call requires a
definitely initialized function value; there is no fallback for text, integers,
arrays or uninitialized locals. Console authority comes from resolved console
operations across all bodies, including unused functions and dead code.

Indirect calls reuse the iterative call machinery, preserving argument order,
caller stack prefixes, isolated locals, result kinds and existing frame/fuel
limits. They add no heap or callable arena. All values are cleared at execution
boundaries, and prior acknowledged console effects survive a later refusal.
Callable parameters, callable results, captures, foreign targets, builtin
references and casts from text or addresses remain outside the profile.

The pinned VM revision `4dad0512dfd3fa4e351f2816196cb48956f5442e` represents
function references as strings. The new independent C++ host therefore tracks
provenance separately: only `PUSH_FN_PTR` introduces a tag, and stack/local moves
carry it. Every callback checks stack/frame shape and tagged strings against the
actual VM. The monitor does not compute arithmetic, text, branches, output or
return values. Same-byte ordinary text cannot masquerade as a function value.
The pinned module parser supplies original parameter names separately from the
executable parser; both parse the same unchanged edition. Prior oracles and
proof suites remain untouched.

## Development findings retained

Artifacts live outside Git under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-callable-2026-11-13/`.
The initial focused run retained two failed expected-error assertions in
`development/flow-tests.log`. Existing identifier parsing classifies a dollar
name in an ordinary identifier position as syntax, so old-profile generated-local
`CALL` and profile-6 `PUSH_FN_PTR` with such a target must expect syntax rather
than unsupported. Tests were corrected without changing parser behavior. A
shared test helper also changed to explicit `ADD i64`, so profile 2 reaches the
intended refusal instead of stopping at an unrelated bare-ADD restriction.

The rerun passes all 99 conformance flow tests and 67 conformance executor tests.
The first complete differential run passes 103 boundaries and the expected nine
raw corpus invocations. All 19 new oracle guard cases pass. The oracle header
was clarified before the final source freeze; those edits changed comments only.
No failed preliminary output is overwritten.

## Evidence contract and source freeze

The final run starts at `e9c6037f6aa5e968f7f4f9fa4aec0230cb3e403f` on
`codex/nil-callable-dispatch`. Its 698 non-Markdown source files are pinned by
`qualification/source-sha256.json`, SHA-256
`6cf33fa91aef5bace266a1b708fb6af4332d14f13e17cb82015130fcd545c341`.
Implementation, probes, proofs and oracle sources remain unchanged during final
qualification. Documentation and external audit files are completed separately.

The qualification plan preserves all six earlier proof suites (126, 77, 61, 27,
55 and 87 cases), their 11 immutable proof/oracle sources, and the unmodified
upstream runner. It retains all 97 compiler calls, 38 roots, 45 interpreter
invocations, auxiliary obligations and the existing telemetry exit-only
exception. No intent is injected and no declaration or instruction is removed.
Final measured evidence and audit identities are recorded below.

## Independent language evidence

All seven final proof suites pass. The new 103-case suite records 40 exact
semantic matches, two matching faults, seven finite resource-policy contrasts
and 54 validation refusals. It includes 49 full-i64 C++ VM comparisons, 103
production counterparts, 49 prior-profile raw/production pairs and one new
corpus prior-profile pair. Refusals and finite policy contrasts receive no
semantic-match credit.

The complete 7,278-byte `paragraphs.matrix` edition (invocation 31) has SHA-256
`4e3da892139e5201831e51c7638d2420702b9ef09de66867810818b02d92a41a`.
It returns zero after 341 candidate instructions, with 13 acknowledged console
calls and 42 byte-exact output bytes. All earlier eight raw matches remain.
The nine matching invocations include six of 38 roots; the 36 remaining refusals
are 27 unsupported and nine edition errors. Production independently refuses
all 45: 24 unsupported, nine edition and 12 intent errors. No corpus input,
annotation, helper or expectation changed.

The final callable proof summary SHA-256 is
`b7e9a3dca586d86c55756fddcc9e1f72923b44a61090e27e15a392b6124cea63`.
The independent retained-evidence summary SHA-256 is
`f5ef125ef365cf78850bd17016f7fdb9c6d2a9631a00d0d433e6aef143dd7f98`.
Raw candidate SHA-256:
`b13b328704d34fcadb7c58c84b40600f54859b0ce8106ec6790daaebd94fbdd5`.
Production candidate SHA-256:
`3040eace3e84b909fb3771d604a6a880dc21ad1d43528d5f8a093e6d6c0c12cf`.
Independent C++ oracle SHA-256:
`3edfb5a640e38ddd94f2372205e8cfe2669eff9264141f43382114f1c3e30c85`.

Independent audits inspect exact retained inputs, process records, console bytes,
full-width results, source identities and pinned archives without importing the
new proof harness. The three oracle-guard jobs retain 30 checks: one text,
ten array and 19 callable checks. The new monitor checks cover legitimate
aliases and shadowing, literal/computed same-byte name forgery, fallback and
caller-capture refusal, frame-local isolation, forbidden consumers, dead target
validation and finite frame/operand caps. Guard checks are safety evidence,
not candidate semantic coverage. Host-only feature exclusion also passes.

Fresh native manifest boots at 96 and 320 MiB retain four admissions, six
refusals, 12 reads, one fixed loader marker and zero executed NIL editions.
Independent transcript verification rejects 8,614 hostile mutations. All five
native admission artifacts remain byte-identical to the prior checkpoint.
This is fresh TCG evidence; historical AMD KVM coverage remains conditional on
exact artifact/source identity. No fresh KVM or VirtualBox run is claimed.

## Next bounded gate

A read-only audit of all 45 retained editions recommends bounded nominal records
with declared `i64` fields as the next slice. Three complete editions (invocations
3, 6 and 34) could raise raw coverage from nine to 12 of 45, if independently
proved. They require declared nominal type identity, `NEW_STRUCT`, `GET_FIELD`,
`SET_FIELD` and record-scoped context metadata separate from main intent.

The pinned VM uses shared mutable record identity: mutation through one alias
must be visible through another. Missing scalar fields default to zero, and a
field write consumes its value while retaining the record. A future record
profile must also replace or extend the compact closed-vocabulary signature key
without collisions between nominal record types. The audit compares globals,
scalar extensions and enums as alternatives; each offers one projected match
with its own additional semantics. None is implemented or credited here.

The external `next-slice-audit.json` pins exact inputs, source evidence and proof
gates with SHA-256
`89ed10e22c442f0a1d1bad5b60f79e1777df19952727284370549984098118dd`.

## Retained host validation

The final `make check` host sections record 81 default / 99 conformance flow
tests, three compile-fail witness checks, 60 default / 67 conformance executor
tests and one compile-fail dispatch check. The broader suite retains 648 default
/ 731 all-feature kernel tests and every intervening feature combination,
425 tooling tests, 37 native-runtime tests and 18 manifest tests. Python harness
selftests record six baseline, five flow, four scalar, five conformance, five
text, six array and seven callable checks. An independent scan confirms those
exact counts and successful host-test results. Full job acceptance additionally
requires the remaining freestanding builds and guest smoke tests below.

## Accepted qualification

All **38 final jobs passed on their first attempt**, with 1,318.1 seconds of
recorded serial execution. This includes all seven proof suites, three oracle
guard jobs, host-only feature exclusion, native manifest admission, `make check`,
all required boot/smoke targets, normal exclusion, manifest proof, native delivery
and Linux loader entry. `make check` took 556.1 seconds. Independent verification
passed the complete retained evidence and exact command inventory.

`qualification/summary.json` has SHA-256
`7463263af081b37c36ab695c0e81ffffdade5e6213cc326494a9e4fdd789cb0e`.
The native delivery boot image remains
`00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b`.
Both Linux runs retain 12 requests, 11 returns and unanswered sequence 12:
`mmap(0x100028000,0x189000,5,0x812,3,0x28000)`. No Linux exit or new compatibility
behavior is claimed.

The original checkout's tracked diff remains
`28d1d1f342846a1cbf8e637b0de1e6eedf71cfccd600e0825b347edce77408c3`,
and all seven unrelated files remain unchanged. This isolated worktree checkpoint
is committed and pushed separately. Full corpus qualification and native NIL
execution remain false; the evidence qualifies only this bounded host slice.
