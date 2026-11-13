# Bounded nominal records — 2026-11-13

This checkpoint extends the host interpreter with explicit profile 7 under
[ADR 0111](../../docs/adr/0111-add-bounded-nominal-integer-records.md). It adds
declared records with `i64` fields, shared mutable identity, field reads/writes,
and nominal helper arguments/results. The immediate targets are three complete,
unchanged corpus editions: the debug-reading fixture, the geometry-point fixture,
and `record-context.matrix`. This slice changes no kernel, native service,
desktop or Linux-loader behavior.

## Implementation and reference boundary

The edition declares up to 64 nominal layouts, 32 fields per layout and 256
total declared fields. A field name is resolved once into a bounded table;
reachable operations prove membership in the receiver's exact nominal layout.
Even unused declarations and dead references remain subject to whole-file
syntax and name validation. Equal layouts with different names remain distinct
types. Every field is exactly `i64`; nested records, arrays of records, text
fields, classes, inheritance and pointer-derived identity remain outside this
profile.

The private executor handle refers to one descriptor and nominal layout.
Copies alias the same cells. `NEW_STRUCT` initializes fields to zero;
`GET_FIELD` consumes its receiver and produces an integer; `SET_FIELD` consumes
the integer and leaves the mutated receiver on the stack. Helpers can mutate
arguments and return records without invalidating their storage. The run owns
128 descriptors and 1,024 cumulative field cells. Empty records consume a
descriptor. Storage is retained until execution ends, then cleared on both
successful and refused outcomes. Exhaustion is reported as `record_limit`.

Function signatures now use fixed exact type-code sequences plus arity,
replacing the prior packed scalar key. Nominal IDs remain distinguishable at
callable joins, including the last layout ID; `i32` and `i64` declarations also
remain distinct. The canonical callable identity still fits in one byte and
the abstract value kind remains two bytes. Earlier profiles keep their grammar
and admission contracts.

Record annotations attach to their record and never become main-function
intent. The opt-in raw conformance path can execute an edition without main
intent, while production admission still requires it. No corpus annotations
are injected, and no unused helper or unsupported form is removed.

The pinned VM revision `4dad0512dfd3fa4e351f2816196cb48956f5442e` stores
record fields in shared `StructData`. Missing scalar fields read as zero and
mutations are visible through copies. Its general record operations are more
permissive than this profile. The new C++ oracle therefore guards declared
membership, nominal argument/result identity, integer writes and bounded
allocation while leaving actual field operations and output to the VM. It
retains the previous callable provenance monitor and preserves every earlier
proof/oracle source unchanged.

## Development findings retained

The initial focused checks passed 115 flow tests plus three compile-fail docs,
and 75 executor tests plus one compile-fail doc. The first differential run
passed all 115 boundaries and all 12 expected raw corpus matches. All 42 new
oracle guard cases passed. The 13 earlier proof/oracle source hashes remained
unchanged.

Independent runtime review found an allocation-order edge case: a callee with a
full 256-value preserved caller prefix could allocate a record before its push
failed. When the record arena was also full, this would report `record_limit`
instead of `stack_limit`. Record construction now checks the operand slot first.
The added regression exercises both an empty arena and 128 existing records;
the focused rerun passes all 76 conformance executor tests plus its compile-fail
doc. Earlier development process records are retained separately; candidate
binaries were rebuilt. Only the frozen final artifacts serve as immutable
qualification evidence.

## Evidence contract and source freeze

The final run starts at `a62b5d5c017b0c6e8210604820edf930c4743352` on
`codex/nil-records`. Its 705 non-Markdown source files are pinned by
`qualification/source-sha256.json`, SHA-256
`9b8026f510c6c455fdb0b246c6efc448d65e387373f92d4e5dfeb34f13054cb2`.
Implementation, probes, proofs and oracle sources remain fixed throughout the
final run. Documentation and external audits are completed separately.

The qualification plan preserves all seven earlier proof suites (126, 77, 61,
27, 55, 87 and 103 cases), their 13 immutable proof/oracle sources, and the
unchanged upstream runner. It retains all 97 compiler calls, 38 roots and 45
interpreter invocations, auxiliary obligations and the existing telemetry
exit-only exception. No annotation, helper or expectation is removed or
rewritten to admit an otherwise unsupported edition.

## Independent language evidence

All eight final proof suites pass. The new 115-case suite records 42 exact
semantic matches, one matching fault, five finite resource-policy contrasts and
67 validation refusals. It includes 48 full-i64 C++ VM comparisons, 115
production counterparts, 48 prior-profile raw/production pairs and three new
corpus prior-profile pairs. Refusals and resource-policy contrasts receive no
semantic-match credit.

The complete newly supported editions are:

| Invocation | Edition | Bytes | Output | Candidate instructions |
| --- | --- | ---: | --- | ---: |
| 3 | debug-reading fixture | 1,395 | `14\n` | 28 |
| 6 | geometry-point fixture | 1,171 | `7\n` | 26 |
| 34 | `record-context.matrix` | 1,684 | `40\n0\n` | 36 |

All three return zero. Their SHA-256 identities, respectively, are
`96814aa904a897e5260df03fd4924387111d2032c6bd68caa210ac4d4eb8f5a1`,
`b97874de3e896b9db7c175c26d1fecd522590d0daf98f7978159481b411d396b`
and `557d9c5b67a2e88a920a62afab535a3c2d646a0be39423fd20f95b18dff04b1a`.
The first two produce one console call each; the third produces two. All nine
prior raw matches remain. Twelve of 45 invocations now match, including seven
of 38 roots. The remaining 33 raw refusals are 24 unsupported and nine edition
errors. Production independently refuses all 45: 21 unsupported, nine edition
and 15 intent errors. No corpus bytes or authored metadata changed.

Final record proof summary SHA-256:
`9a60109c4910a104c917f4634b6593dfd46e2bde18fb338fb88e334924a9e8ce`.
Independent retained-evidence summary SHA-256:
`096494758d13814094fd6fc812e2e8881414831b51f33a4461dbf114b56ef191`.
Raw candidate SHA-256:
`e44ac7915771446f22cf41eacc14f61c8982025a5b62e1e9d6b84856a9122b46`.
Production candidate SHA-256:
`39e5aca648d045160264c90ae2c30c50caca984f35dd729d88583b29a32c1c73`.
Independent C++ oracle SHA-256:
`0eafa61cf8531069f548eb4317d6f0b066b516ae7d23583cfbe8d8459bc64ad0`.

Independent audits inspect retained inputs, process records, full-width results,
console bytes, binary/source identities and pinned archives without importing
the new proof harness. All four oracle-guard jobs pass: one text, ten array,
19 callable and 42 record checks. The new checks cover real shared identity,
field/type guards, nominal calls and returns, forged/scalar/array consumers,
schema boundaries and explicit larger oracle allocation limits. They count as
72 guard fixtures, not 72 semantic corpus matches.

## System qualification

All **41 final jobs pass on their first attempt**, totaling 1,382.1 seconds of
serialized work; `make check` accounts for 584.1 seconds. The run includes all
required AGENTS smoke targets, all eight language proofs, all four oracle guard
jobs, native admission/delivery, manifest qualification and Linux-loader entry.
No memory or timeout guard fired.

The independent aggregate verifies exact commands, process results, all 651
boundary cases, 72 oracle guards, complete source/tool/input identities and
original-checkout preservation. Final host counts are flow 95/115 and executor
68/76 (default/conformance), kernel 648/731 (default/all features), xtask 425,
runtime 37 and manifest 18. The three flow and one executor compile-fail
witness tests pass. Eight Python harness suites pass 6/5/4/5/5/6/7/7 self-tests.
Freestanding builds reject both host-conformance feature paths as required.

Retained development and final artifacts live outside Git under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-record-2026-11-13/`.
The final aggregate `qualification/summary.json` has SHA-256
`e8567d2892a9c660d0e005f9f314d674dbe060d869d1c46fc80310473e106553`.
All 705 frozen source files remain unchanged. The original tracked diff and
seven unrelated files are preserved.

## Native continuity and remaining gates

Fresh TCG native-admission runs at 96 and 320 MiB retain four admissions, six
refusals, twelve reads, one fixed loader marker and zero NIL execution. The
independent native verifier passes all 8,614 mutation checks. Boot image, kernel,
client, RX image and payload bytes match the reviewed checkpoint exactly;
historical AMD KVM evidence is reused only under those exact identities. This
slice claims no fresh KVM or VirtualBox run.

Native program delivery retains its reviewed boot-image identity. Fresh
Linux-loader runs at both memory sizes still record twelve requests and eleven
returns, ending at unanswered sequence 12:
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
There is no Linux program exit or new compatibility claim.

Native NIL execution, full corpus qualification, new native service authority
and general Linux compatibility remain separate closed gates. This host-only
extension does not make the desktop run record programs or change Linux-loader
behavior. Production still requires authored main intent, and the original
user checkout remains outside this branch's edits.

A read-only audit of all 45 final retained editions identifies bounded `i64`
module globals plus integer-to-text casts as the next behavioral candidate. It
could admit the complete `two-books.matrix` edition (invocation 43), moving to
13 of 45 only after a new proof. The other small global example still contains
an unsupported unused enum declaration, so it earns no projected match. Scalar
extensions must likewise validate and exercise forms in uncalled helpers.

The audit is retained as `next-slice-audit.json`, SHA-256
`bf9fdbb7b8a55f6778fe08268dfda28d3e230e6e214a7cfc14f58a7aafd61fc6`.
Source-debug metadata and binary NIL v5 are separate format slices. Their
unchanged runner embeds temporary source paths, so exact whole-file hashes can
vary between runs; capture exact bytes per run rather than normalizing those
paths or treating metadata as file-read authority.
