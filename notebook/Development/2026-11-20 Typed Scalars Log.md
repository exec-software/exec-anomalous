---
title: "Bounded Typed Scalars"
date: 2026-11-20
tags: [nil, execution, scalar, bool, u8, f64, conformance]
status: verified
domain: foundations
---

# Bounded Typed Scalars

## Scope

[ADR 0115](../../docs/adr/0115-add-bounded-typed-scalars.md) introduces explicit
host profile 11 for distinct boolean, byte and finite binary64 values. The
complete unchanged `text-and-types.matrix` edition is the target: its unused
`ratio`, `ready` and `pack` helpers require all three types. Merely executing
its integer/text main would not qualify the whole artifact.

The pinned general VM erases bool/u8 to signed integers. Anomalous retains
distinct validated kinds through locals, branches and exact direct/indirect
helper signatures. Conversion is explicit. Integer-to-bool accepts only zero
or one; byte conversion retains the low eight bits. The existing signed i32
narrowing remains unchanged. Float values are finite binary64 bit patterns,
with identity casts and ordinary value movement only. Arithmetic, formatting,
float/integer conversion and new aggregate element types are separate work.

Decimal float grammar follows the pinned numeric parser. Signed zero retains
its sign even with a large syntactically valid exponent. Nonzero underflow to
zero and overflow to infinity refuse; finite subnormals remain representable.
No literal or operation opens files, derives authority or supplies main intent.

## Evidence design

The new reference oracle calls the pinned VM, separately inventories parsed
float bits and observes actual VM values after floating literals, loads and
identity casts. The candidate probe exposes the immutable plan's literal bits.
This makes signed-zero and decimal-rounding checks independent of console
formatting. Private executor tests additionally inspect bits after direct and
indirect helper returns, then verify complete cleanup through the public entry.
The observation field is proof tooling, not an execution capability.

Every preceding proof/oracle source is preserved. All corpus compiler attempts
and all 45 interpreter editions are retained, with separate raw conformance
and production-admission attempts. Native execution remains gated.

The final qualification uses the same retained glibc 2.39-0ubuntu8.9 fixture
bytes through the explicit selectors introduced at the preceding checkpoint.
It neither changes the fixed regression hashes nor substitutes installed host
libraries. Evidence is retained under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-typed-scalars-2026-11-20`.

## Development checks

The focused suites passed: flow 146 default / 173 conformance tests, executor
98 default / 107 conformance tests, and four compile-fail documentation tests.
All 96 independent new oracle checks passed after correcting malformed-literal
expectations to the earlier module parser's diagnostic. A retained rerun stopped
on an existing output directory; its artifacts were preserved before a fresh
successful run. The flow test harness also needed its historical profile-1
error expectation and one owned-string workspace lifetime corrected. These were
test changes, with no scalar semantic change.

The initial complete target comparison matches full signed-i64 result zero,
37 dispatched instructions, six console calls and 200 exact output bytes:
`b9725e336c24624b7a9bd3dd4e52e224df763888686b3e263a719dd1159a9ff2`.
Its unused float helper retains literal bits
`3fe0000000000000` at instruction zero. Production refuses for missing authored
intent; profile 10 still refuses the new syntax. These preliminary checks do
not substitute for fresh complete qualification.

## Fresh language qualification

The final cohort freezes 735 non-Markdown source files at base commit
`421e97f456a66451dec51b22ac9cb1029edb7d73`. Its source inventory digest is
`12e4bc93efd784fceff9f112bd9d22a872149f067adf0a48ba58a524d7027321`.
The new proof source is
`91c03de8a7cbfa41f10e20ea1773de37301a13c5db39d253982c0f8d267b9f73`.
All 21 preceding proof/oracle sources remain unchanged.

All twelve fresh language suites pass: 126, 77, 61, 27, 55, 87, 103, 115,
105, 149, 111 and 200 cases, totaling **1,216**. The new 200 cases comprise
81 semantic matches, ten matching runtime refusals, two finite runtime-policy
contrasts, five finite scalar-policy contrasts and 102 validation refusals.
There are 200 production counterparts, 98 real-reference comparisons,
40 exact literal observations and 96 profile-10 comparisons in each lane.
The harness's ten self-tests exercise receipt and target mutations.

The complete raw run retains all 97 compiler invocations (54 successful and
43 negative), all 38 roots and all 45 interpreter invocations. Matching raw
indices are `0,1,2,3,4,6,17,28,30,31,34,36,37,38,42,43,44`: **17/45**, including
**11/38** roots. All other 28 are unmet coverage, with 19 unsupported and nine
edition refusals. Production independently attempts all 45 exact editions and
refuses 20 for intent, 16 as unsupported and nine as editions. No raw result
waives production admission.

The new complete invocation 42 remains 2,990 bytes with SHA-256
`7477ac54c681108dc9f1c40b604ee5e2ff491ea07277ed0f3001159545f4a2cb`.
Its full signed-i64 zero return, exact output, six complete functions and unused
typed helpers are checked against the actual reference VM. The preserved
source-debug invocation 4 retains 23 records and the exact fresh source path
`/tmp/tmpcpy1dom9/source-debug.matrix`; no temporary path is normalized.

Fresh independent audits confirm all twelve suites. They independently decode
float decimal values with integer-rational nearest-even rounding and check the
entire capture, both corpus lanes and complete targets. All **356 oracle
checks** pass (260 earlier plus 96 new), as do 37 adversarial checks of the new
independent auditor. Altered scalar bits, signed-zero/subnormal loss, missing
metadata and a colluding result 256 with the same low-byte CLI exit are rejected.
Both freestanding builds reject the host-conformance feature explicitly.

| Final retained receipt | SHA-256 |
| --- | --- |
| `typed-scalars-proof/summary.json` | `57c8b590dd8a0eb5dffeadd7ecb76a05f2abd36fc808c9d94d42bed807fbd6e1` |
| `typed-scalars-proof/complete-target-042.json` | `9b21a751a8ced766ee7c7e04699e939eae87d80c319ea0a1bc3712f982500f46` |
| `typed-scalars-proof/complete-target-004.json` | `1012c8b26006640b67272f4a149757d8845a0f2d41582f3f7336c13763d4a37a` |
| `independent-typed-scalars-summary.json` | `ff5e33fffd167241a8e6fcca6858245ca87055d26207e03086aa1f409c2e8700` |
| `implementation-review-final.json` | `1f99f75b601f8a118885684fb1717f95a409a5f5b309f2bb90bd54712feb6ee4` |
| `proof-review-final.json` | `36a7a9bf6353f6cc8c8f0cc8a6c6d5d079749edeb9379a55955c19650c5c2ab7` |

The table's paths are beneath the evidence base's `qualification` directory.
The independent target and next-slice audits are retained alongside that
directory, with final receipt digests
`4f1b5e67ec3674f4f265e02df20b6aeae589962fcd52ba0847a0b1f009d4ae4f`
and `2fe239c40256960a52ee0084c83561e7656709ab362eb3dd2d03b0b30fdb98a4`.

## Native continuity and remaining gates

Fresh TCG native admission at 96 MiB and 320 MiB admits four and refuses six
packages, performs 12 bounded reads and one loader marker, executes zero NIL
editions and restores resources. Its independent verifier rejects all 8,614
mutations; receipt SHA-256 is
`450d251e9fdaa95ea53046dcf1d6fb6472fcb7fc382d5c3b1f8180d65d407dc1`.
Historical AMD KVM evidence remains historical; this cohort makes no fresh KVM
claim. Full-corpus and native execution are still separate gates.

The next textual target is complete `aggregates-twin.ny` invocation 7:
nested nominal record fields plus concrete generic record/enum identities.
The pinned VM returns integer zero for a missing nested field, so future typed
handles need an explicit initialization or fault rule while preserving shared
aliases. Invocation 35 additionally needs hygienic local names; invocation 8
adds allocation and mutable arrays. None is counted as supported here. Binary
NIL v5 and the eight oversized textual editions retain separate format/capacity
gates. No Linux application exit or new guest compatibility is established.

## Complete repository qualification

All **53 fresh jobs pass on their first attempt**, totaling **1,448.1 seconds**;
`make check` takes **588.1 seconds**. No memory or timeout guard fires. All
735 frozen source files remain unchanged. The final cohort contains its own
oracle/candidate builds, all twelve proofs, all eight oracle-guard groups,
freestanding exclusions, native admission, complete repository check, all
required VM smokes, program-manifest proof, native delivery and Linux-loader
regression. Development attempts are retained separately and supply no final
qualification credit.

Final host counts include flow 146 default / 173 conformance, executor 98 / 107,
kernel 648 default / 731 all-features, xtask 425, native runtime 37 and manifest
18. All twelve Python self-test groups pass, including the new ten tests;
the three flow and one executor compile-fail documentation checks also pass.

All five native admission artifacts match the preceding qualified bytes; the
native delivery boot image remains
`00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b`.
The Linux regression uses the unchanged pinned program/interpreter/libc inputs
and verifies 12 requests and 11 returns at 96 MiB and 320 MiB. The observed
unanswered sequence 12 remains
`mmap(0x100028000,0x189000,5,0x812,3,0x28000)`. No policy is executed for that
next request, and neither libc startup nor application exit is established.
The previously qualified input-selection proof remains historical evidence;
this cohort freshly verifies fixture hashes and actual loader artifacts.

The independent final aggregate is `qualification/summary.json`, SHA-256
`4974c222ff100bb295f6dd4c939f5308d2f5abb484d6594a74197cd3538eb898`.
Its read-only verifier is `verify_qualification.py`, SHA-256
`bddd92125c35810cb03c60612b09f6f87e7100095356adca5577fd2a37d6b28a`.
It confirms all jobs, exact source/tool/fixture identities, peer reviews,
original tracked-diff preservation and all seven unrelated original files.
The original checkout's retained diff digest remains
`28d1d1f342846a1cbf8e637b0de1e6eedf71cfccd600e0825b347edce77408c3`.
The independent implementation audit is
`implementation-review-qualification.json`, SHA-256
`b146b3bd705f442b274d36a2e99230fac67e991621aa29bbe7633e74bc5f71ce`;
the reference-source audit is `reference-source-audit-qualification.json`,
SHA-256 `f1feb6b060c8f35d9a1e730bc65785604199bfc18e424f198647acfe44eecfb1`.
All reviews report no open findings.
