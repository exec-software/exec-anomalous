# Bounded nominal enums — 2026-11-13

This checkpoint develops explicit host profile 9 under
[ADR 0113](../../docs/adr/0113-add-bounded-nominal-unit-and-integer-enums.md)
for declared variants with zero or one `i64` payload. The target editions are
the complete unchanged `names-and-case.matrix` and `value-position-then.matrix`
programs. It changes no kernel, native service, desktop or Linux-loader behavior.

## Implementation and reference boundary

Enum declarations retain exact nominal identity, with at most 64 layouts,
32 variants per layout and 256 declared variants in total. Empty enum metadata
is permitted but supplies no constructible values. Every variant is either a
unit or a single integer payload. Layout names use bounded ASCII identifier
segments joined by `::`; a complete layout-and-variant tag fits 128 bytes.
Declarations and opcode metadata are checked across the complete edition,
including unused helpers and unreachable instructions.

The profile reserves terminal enum names `Option` and `Result`, regardless of
case or qualification, and excludes dotted tags. These are explicit bounded
profile restrictions: the pinned general VM canonicalizes those special names
and aliases. Ordinary accepted names retain exact case and spelling. Record
and enum names cannot collide. Enum context belongs to its declaration and
cannot provide main-function production intent or service authority.

`NEW_VARIANT` consumes the declared payload and produces a private immutable
value. `IS_VARIANT` consumes a value of the declared nominal type and returns
integer zero or one. `UNWRAP_VARIANT` consumes the enum and produces exactly its
requested zero or one integer payload. Helpers, indirect callable signatures,
locals and branch joins preserve nominal type identity. Different variants of
the same enum can join; identically shaped different enum types cannot.

Runtime values keep nominal identity, variant identity and the optional full
signed integer inline in existing bounded operand and local storage. They use
no allocation arena or cumulative construction quota. Copies retain the value
through helper returns without mutable aliasing. Existing frame, operand, fuel,
output, authority and complete execution-boundary clearing remain in force.

The reference is compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. The preceding primary probes established
that payload extraction preserves all 64 integer bits, including extrema and
values above exact floating-point integer precision. The new runtime performs
no narrowing or floating conversion.

The general VM tests tags without checking nominal metadata and also permits
legacy strings as enum operands. Its extraction pushes only the requested
prefix of available payloads. Our validator independently proves nominal
identity and rejects forged scalar or text operands. At extraction, the runtime
checks exact payload shape before consuming the owner. A mixed-shape enum can
reach that check through an ordinary join or helper argument; a mismatch returns
`enum_shape` without exposing older caller operands or discarding a payload.
These deliberate stricter policies are separate from matching semantics.

Earlier profiles retain their grammar and envelope tags. Production admission
still requires authored main intent; raw conformance never manufactures it.
No corpus declarations, helpers or metadata are removed to gain coverage.

## Development evidence and source freeze

The focused run passes 146 conformance flow tests and 97 conformance executor
tests, plus three flow and one executor compile-fail witness tests. Both focused
Clippy checks pass. Three independent source reviews find no implementation
defect. The enum-shape cleanup test retains live computed text, array, record,
global and enum state before the fault and verifies complete clearing.

The first flow test attempt exposed three new fixture mistakes: an old-profile
post-return instruction expected the wrong first error, a branch label collided
with a helper function, and a comparison incorrectly carried a width operand.
The corrected fixtures pass without changing parser behavior. Two proof-fixture
first-error expectations were also corrected before executable comparison.
Retain the initial failed flow log; it receives no qualification credit.

All 149 development proof cases and 73 new oracle guards pass. The seven proof
selftests pass, including rejection of substituted reference errors and policy
contrasts misclassified as semantic matches. An independent retained-evidence
audit confirms all boundary results, complete corpus lanes and guard fixtures.
Its development receipt has SHA-256
`b62d5bc49eda395d73f716b16e35bfcbc4842c6a106840fc4f92df8db4ba09b2`.

The final run starts at `b39340b7f0ea3e9c978e0f794cad6abccf803ea5` on
`codex/nil-nominal-enums`. Its 718 non-Markdown source files are pinned by
`qualification/source-sha256.json`, SHA-256
`40ca0b14405f9346c0e679afb0a6bc854f59bba834c73d1a186b8a2e83e08c84`.
Implementation, probes, proof scripts and oracle sources remain fixed during
qualification. Documentation and external audits are completed separately.

Retained development and final evidence belongs outside Git under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-enum-2026-11-13/`.
All 47 final qualification jobs pass. The final measurement retains 15 of
45 raw invocations and 10 of 38 roots, independently audited below.

## Independent language evidence

All ten final language proof suites pass: 126, 77, 61, 27, 55, 87, 103, 115,
105 and 149 cases, totaling 905. All 17 earlier proof/oracle sources remain
byte-identical. The new suite records 48 exact semantic matches, one matching
runtime fault, four finite policy contrasts and 96 validation refusals. It
includes 53 full-i64 VM comparisons, 149 production counterparts, 53 previous
profile raw/production pairs and two complete corpus previous-profile pairs.
Policy contrasts and validation refusals receive no semantic-match credit.

Both newly supported complete editions return zero:

| Invocation | Root program | Edition bytes | Candidate instructions | Console calls | Output bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| 30 | `names-and-case.matrix` | 3,769 | 132 | 8 | 51 |
| 44 | `value-position-then.matrix` | 4,057 | 65 | 4 | 19 |

Invocation 30 has edition SHA-256
`5fed5a7122c09ad2d597072efdf3ac856ca8372019015b68130df8be9465d385`.
Its output is `1\n2\n3\n4\n3\n2\n4\nan identifier that had to be escaped\n`.
The whole unused `frame_kind` enum declaration is validated alongside the
initializer, globals, record helper and main. Invocation 44 has edition SHA-256
`bac98e0cba04770cab9f67dd290f9180d48c08da37f94c1d5b683d4cab9ad276`.
Its output is `many\nmany\ntwo\nbusy\n`; it executes variant construction, tag
testing and integer extraction. No source bytes are rewritten.

All 13 earlier raw matches remain. Fifteen of 45 raw invocations match,
including 10 of 38 roots; the other 30 are 21 unsupported and nine edition
refusals. Production independently refuses all 45: 18 unsupported, nine edition
and 18 intent errors. The unchanged runner still records 97 compiler calls,
38 roots and 45 interpreter invocations, with all auxiliary obligations and
the existing telemetry exit-only exception preserved.

Final enum proof summary SHA-256:
`9dfac5b874fdd495f3d47cb71a72c16fe2c0f95ea7deebb053a056093338ace8`.
Independent retained-evidence summary SHA-256:
`f1d9d28663921dabd8bc1698cfad7a5d71b885ff555247c2cb861cb237cac009`.
Raw candidate SHA-256:
`98263309feecacf8d412cb9d96c530a48a41723fcd563d7d56193ff6bac925a9`.
Production candidate SHA-256:
`be61c9a98f6df52fba256c270a2043b7e9aee55a7e52de2a4079f9f957294f85`.
Independent C++ oracle SHA-256:
`a0b967a4ba638042071934b0faac04c60bc6efdc3d410d1235d6bf157fef1fbd`.

The six oracle-guard jobs pass one text, ten array, 19 callable, 42 record,
43 global and 73 enum fixtures: 188 total. The new guards check declared
identity, actual payload width and kind, opaque consumers, schema bounds,
canonical alias exclusions and actual permissive extraction. Both freestanding
host-conformance exclusions pass. These are guard checks, not additional corpus
matches.

## System qualification

All **47 final jobs pass on their first attempt**, totaling 1,552.1 seconds
of serialized work; `make check` accounts for 660.1 seconds. The run includes
every required AGENTS smoke target, all ten language proofs, six oracle-guard
jobs, native admission/delivery, manifest qualification and Linux-loader entry.
No memory or timeout guard fired. No boot implementation changed; no new
VirtualBox or KVM run is claimed.

The independent aggregate verifies exact commands and results, all 905 boundary
cases and 188 oracle guards, complete source/tool/input identities and original
checkout preservation. Host counts are flow 122/146 and executor 88/97
(default/conformance), kernel 648/731 (default/all features), xtask 425,
runtime 37 and manifest 18. Three flow and one executor compile-fail witness
tests pass. The ten Python suites pass 6/5/4/5/5/6/7/7/7/7 selftests.

The final aggregate `qualification/summary.json` has SHA-256
`a5175008d5c46e3b44fea00d03ffa9e393b95e077cca94e0278e30ea3e587c8d`.
All 718 frozen source files and all 17 previous proof/oracle sources remain
unchanged. The original checkout's tracked diff and seven unrelated files are
preserved. Documentation links and the final source freeze are checked before
committing the completed slice.

## Native continuity and remaining gates

Fresh TCG native admission at 96 and 320 MiB retains four admissions, six
refusals, twelve reads, one fixed loader marker and zero NIL execution. The
independent verifier passes all 8,614 mutation checks. Boot image, kernel,
client ELF, client RX span and payload object remain byte-identical to the
previous checkpoint and historical AMD KVM-qualified artifacts. This is fresh
TCG evidence and exact historical continuity, not a new KVM run.

Native program delivery also passes at 96 and 320 MiB with its prior artifact
identity. Linux-loader entry preserves twelve requests and eleven returns at
both sizes, stopping at the unanswered sequence-12
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
No Linux application exit or further syscall support is established.

The read-only final target review has SHA-256
`f9d6d0b5b96a1af29b08ce13b8ab0dae9fe196951a748b78626eb61893919a34`.
The next-slice audit pins all 30 remaining refused editions and has SHA-256
`57fbcda7357e608ecc9ea5b470667a432f0a19480cadcb6a0875406a93de4efa`.
Its smallest proposed checkpoint is strict textual source-debug metadata,
potentially admitting one additional auxiliary invocation with no new root.
The next runtime expansion is a carefully bounded bool/u8/f64 subset for one
complete root whose unused helpers must still validate. Those are future
projections, not coverage claims. Binary NIL v5, oversized editions, generic
and nested aggregates, classes, ownership, time and foreign effects retain
their separate gates.

Full unmodified corpus qualification, native NIL execution, production intent
admission and general Linux application compatibility remain unqualified.
