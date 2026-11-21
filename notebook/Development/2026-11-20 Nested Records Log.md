---
title: "Bounded Nested Records"
date: 2026-11-20
tags: [nil, execution, records, generics, conformance]
status: verified
domain: foundations
---

# Bounded Nested Records

## Scope and policy

[ADR 0116](../../docs/adr/0116-add-bounded-nested-records.md) introduces explicit
host profile 12 for nested nominal record fields and concrete generic record
and enum identities. The complete unchanged `aggregates-twin.ny` invocation 7
is the target. Its nested `segment` fields, `envelope<i64>` layout and
`maybe<i64>` variant metadata must all validate without removing unused bodies
or rewriting compiler output.

A field is either i64 or an exact declared record kind. Bounded resolution of
all field type names permits forward, self and mutual references. Runtime
fields hold integers, uninitialized markers or shared record handles; they do
not embed child records. Cycles therefore require no recursive allocation,
traversal or cleanup. The existing 128-descriptor and 1,024-field budgets stay
cumulative for the complete run. Reassigning a parent field preserves older
aliases to its former child.

The pinned VM returns integer zero for an absent ordinary field, even when its
declaration names a nested record. Anomalous instead reports
`record_uninitialized` for an unset record field, without exposing an older
stack operand or forging a handle. Integer fields retain the existing zero
default. This is an explicit runtime-policy contrast with the actual reference.

Concrete names permit one terminal primitive argument: i32, i64, bool, u8,
f64 or text. Full declared names retain exact nominal identity. There is no
template instantiation, inferred substitution, nested argument or argument list.
Generic enum tags omit the suffix while appended metadata names the complete
concrete type. Candidate type checks remain stricter than the reference VM's
tag-only variant test across different concrete instances.

Review found special handling for ptr/ref wrapper syntax in the reference
loader and VM. Profile 12 explicitly reserves the six unqualified generic bases
ptr/ref/owned/borrow/fn/tuple, case-insensitively, when suffixed. Existing plain names and
ordinary qualified names remain available. This refinement is qualified in a
fresh development cohort before the final source freeze.

Text, boolean, byte, float, enum and array record fields; hygienic local names;
allocation/free instructions; classes; broader generic syntax and native NIL
execution remain separate work. No added type or metadata grants a service or
supplies authored main intent.

## Development evidence

The initial focused run passes all seven jobs, including 105 independent oracle
checks, flow 159 default / 187 conformance tests and executor 109 / 118 tests.
The complete retained target matches full signed-i64 result zero, 150 dispatched
instructions, nine console calls and 51 output bytes with SHA-256
`f39c01ba0848f6d4f3a44f178f7658d8274e149e5802ffba9c0800a82c9059c4`.
Production refuses missing authored intent and profile 11 refuses the new syntax.
The initial results precede the wrapper-name and tag-length policy refinement;
they remain separate from the revised and final evidence.

Evidence is retained under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-nested-records-2026-11-20`.
Final qualification uses explicit retained glibc 2.39-0ubuntu8.9 inputs with
unchanged fixed Linux regression hashes, following the preceding checkpoint.

The revised `development-2` cohort passes all nine jobs on its first attempt,
including 122 oracle checks, flow 160 default / 188 conformance tests, executor
109 / 118 tests, nine new proof self-tests and all 161 boundary cases. The
independent audit confirms 18/45 raw matches and 12/38 roots. Fifty-one
adversarial checks of the independent auditor reject altered alias/cycle results,
missing-field policy changes and a full result 256 masquerading as zero through
a process exit byte. These are separate from actual-VM oracle coverage.

The final cohort freezes 742 non-Markdown files at base commit
`140e730c23337700cad542500322506900aaf5fb`; source inventory SHA-256 is
`3ba5c7ad2c0c718a3a75d943b184d8c5213bf6f245f42bb1138f303f62d52aac`.
All 23 preceding proof/oracle sources remain unchanged. The new proof source is
`3f0c19c623f3745349aeb306d0d199907abd4429851ec271796f379101040a3c`;
the new reference source is
`59a1bf0b4272a65bca19cfef4be5074810bfbea785b3e723bd3a874b6142ebbc`.

## Final language evidence

All thirteen final language suites pass independent retained-evidence checks:
1,377 boundary cases and 478 actual-VM oracle guards. The new suite contributes
161 cases: 65 semantic cases, eight runtime-policy contrasts, one matching
division fault, two erased-tag admission-policy contrasts and 85 validation
refusals. Its 76 actual-VM comparisons and 74 prior-profile comparison pairs remain
separate from the 51 adversarial checks of the independent auditor.

The final authored nested-record summary has SHA-256
`6e4b5c2943cffe317688935ccee7d257bc68c1df216a8d3a78d3fc2f3d6e29e9`;
the independently reconstructed summary is
`c00a1d963ebb6e9f9cb1d95ebc93c06fd551543c3f7cade3ec93c37e01ef5ee5`.
The final reference/source audit is
`2b0a39f9028783a4bf3d245d99dbc7f215d69e7cd63bdf518e91917af3f7fdb7`.

The fresh complete invocation 7 capture remains 7,534 bytes with SHA-256
`70021ba440a8fe8c050607325a6b6d250bbfa5e3e19077bf6cee86b43eb7bef7`.
Its full signed-i64 result is zero, with 150 instructions, nine console calls
and the same 51 output bytes reported above. Both unused and used declarations
remain present. Complete prior targets 4 and 42 also retain their metadata and
behavior. The fresh source-debug path is `/tmp/tmp0f47jkr1/source-debug.matrix`;
all 23 records across three functions retain its exact bytes. No temporary
path normalization or reused previous capture is counted as this run.

All 45 unchanged raw invocations and all 45 production attempts are recorded.
The 27 raw refusals are 16 unsupported, two syntax and nine edition refusals.
Production retains 45 refusals: 13 unsupported, two syntax, nine edition and
21 missing-intent refusals. Corpus capture still records all 97 compiler calls
(54 successful and 43 negative), 38 root cases and 45 interpreter invocations.
Neither partial coverage nor expected refusal is full corpus qualification.

The complete-target audit is
`28e4b329b3e08264c45ce7b1d6afc9b5c23d369671b73792367a3dbb01f90d15`;
the next-slice audit is
`b6288031813b9e9934498085c28e6a253ced60a3aa0fb4d5ca14bb0b331aa370`.
The implementation review bound to these final receipts is
`06b8a05491f8bc96b5355163b44c4aed6fd0c33183defd74c958697c601c876b`.

## Final qualification

All 56 fresh jobs pass on their first attempt, with no timeout, memory guard
or source change. Summed job time is 1,790.1 seconds, including 600.1 seconds
for `make check`. The independent aggregate verifies all language suites,
oracle guards, repository checks, required smoke tests and retained artifacts.
Its `qualification/summary.json` has SHA-256
`d6d09f0a5e3cbaed77d102521dd3543e60ae991400a7bbf31d28ba913c53b0b2`.

Host counts are kernel 648 default / 731 with all features, xtask 425,
native runtime 37, manifest 18, flow 160 default / 188 conformance and executor
109 default / 118 conformance. All nine new proof self-tests pass. The explicit
host-conformance feature remains unavailable on freestanding targets.
All 742 frozen non-Markdown files and all 23 prior proof/oracle sources retain
their recorded hashes. The pinned reference checkout stays clean at revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. The original Anomalous checkout's
seven unrelated files and tracked-diff hash remain unchanged.

## Native continuity

Fresh Intel TCG admission boots at 96 and 320 MiB each retain four admissions,
six refusals, twelve reads, one loader marker and zero NIL executions, with
complete cleanup. The five pinned native artifacts remain byte-identical.
The fresh native verifier receipt has SHA-256
`14828003130ae56ca96939052e12a01ec58acd5226cc8ef978afc378818530da`,
including 8,614 rejected mutations. Prior AMD KVM evidence establishes only
continuity with those exact artifacts; this checkpoint makes no fresh KVM claim.

Native program delivery also passes with the unchanged boot image. The Linux
regression retains its fixed glibc inputs and the same boundary at both memory
sizes: twelve requests, eleven returns, then the observed sequence-12
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`. Containment and cleanup
pass; this slice adds no Linux application completion or libc-startup claim.

## Remaining gates

The next focused host slice is the compiler's canonical hygienic local identity
`$ny.local.<counter>.<name>`. Invocation 35 now reaches that refusal after its
nested and generic declarations validate. Its complete 7,966-byte edition has
SHA-256 `19fb31d7b8a1a3eedafc48337d4f5a616aa8cfc9009978e982c7e459bafabe79`.
Support would still need a fresh complete-program proof before claiming a
nineteenth raw invocation or thirteenth root. Invocation 8 additionally needs
mutable storage and allocation. Binary NIL v5 and the eight oversized textual
editions remain separate format and capacity work. Native NIL execution and
broader Linux application support remain gated.
