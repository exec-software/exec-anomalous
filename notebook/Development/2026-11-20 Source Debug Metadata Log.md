# Bounded textual source-debug metadata — 2026-11-20

This checkpoint develops explicit host profile 10 for validated descriptive
source paths and lexical binding snapshots under
[ADR 0114](../../docs/adr/0114-add-bounded-textual-source-debug-metadata.md). Its target is the complete textual
source-debug auxiliary invocation from the unchanged compiler corpus. The
preceding profile-9 checkpoint is `aaa6cb7501046660d8b19c9c9fa9ccf86e71ac7d`.
Kernel, native service, desktop and guest Linux-loader implementations are
unchanged. Host regression input selection is extended as recorded below.

## Implementation and reference boundary

A reserved `# source_debug` record belongs to the next actual instruction and
must carry that instruction's exact zero-based index within its function.
Labels count as instructions; metadata and blank lines do not. Validation checks
all records, including those in unused helpers and after unreachable branches.
Malformed, duplicated, misplaced and orphaned records refuse the edition.
Unknown comments retain the existing strict grammar.

Each record has a nonempty quoted path and a bounded ordered list of lexical
name, storage name, type description and scope depth. Decoded paths fit 16,384
bytes; each binding string fits 128 bytes. At most 128 bindings belong to a
record, with scope depth at most 1,024. Lexical names and storage names are each
unique within the record. Only escaped double quotes and backslashes are
accepted; the complete edition retains its UTF-8 and control-character checks.
Existing 65,536-byte edition and 4,096-instruction bounds still apply.

The plan stores one optional borrowed encoded record per instruction. Read-only
views expose the path and binding iterator, with decoded byte iteration and
atomic copying into caller storage. No per-record binding arena is retained.
Paths remain data: neither validation nor execution opens them. Descriptive
bindings need not name runtime locals, and their type strings cannot change
operand kinds. Metadata supplies neither authored main intent nor service
authority and consumes no execution fuel.

The reference remains compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. A new oracle uses its real parsers and VM,
compares every static source record between the module and executable parsers,
checks that generated scaffolding has none, and verifies metadata survives VM
loading. Both candidate probes and the oracle expose canonical decoded metadata
snapshots alongside execution results. Runtime faults retain that static
snapshot; admission or validation refusals expose an empty list.

The pinned parser permits additional operand spellings, unknown escapes, larger
binding counts and binding strings. Those cases are explicit finite-reference
policy contrasts. The candidate's existing whole-edition control check also
rejects C1 control characters accepted by the reference's debug UTF-8 helper.
Neither policy contrasts nor validation refusals receive semantic-match credit.

Earlier profile tags, grammars and proof sources remain immutable obligations.
Production continues to require authored main intent; raw conformance relaxes
only its absence. Complete source artifacts retain unused helpers and original
metadata. Temporary source paths are never normalized to stabilize hashes.

## Qualification evidence

Retained development and final
artifacts belong outside Git under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-source-debug-2026-11-20/`.
All 13 development jobs pass, including 159 conformance flow tests and 100
conformance executor tests, three flow and one executor compile-fail witness
tests, both focused Clippy checks, nine proof selftests, the 111-case proof and
72 independent metadata/VM guard fixtures. No implementation defect was found
by the independent source reviews.

Pre-execution review corrected two fixture expectations: a bare SUB spelling
was changed to the intended typed operation, and the prior-profile C1 control
case was made to expect the inherited syntax refusal. The first development
proof passed; subsequent review strengthened the new corpus target to compare
its full integer result as well as its process status. A ninth selftest rejects
low-byte result aliases, rewritten source paths and changed capture evidence.
A fresh revised proof passes with the original run retained separately. Neither
review change alters the parser or executor.

The first qualification run starts at `aaa6cb7501046660d8b19c9c9fa9ccf86e71ac7d` on
`codex/nil-source-debug`. Its 726 non-Markdown source files are pinned by
`qualification/source-sha256.json`, SHA-256
`5bf678aad5ff0193e3703d816ab7498859106a3af2e71afca7fbdbc25c1adf46`.
This first freeze is retained with its incomplete qualification cohort below.
The fresh final freeze additionally includes the explicit input selectors.

The revised development measurement passes 37 exact semantic matches, one
matching runtime fault, one finite runtime policy contrast, 14 finite metadata
policy contrasts and 58 validation refusals. It includes 53 actual VM/reference
comparisons, 111 production counterparts and 52 earlier-profile raw/production
pairs. The complete source-debug auxiliary edition additionally has one prior
profile pair. Nineteen earlier proof/oracle sources remain unchanged.

The complete corpus now matches 16 of 45 raw invocations, including the same
10 of 38 roots. The new auxiliary invocation returns zero and emits `14\n`
in 28 candidate instructions, one console call and three bytes. All 23 metadata
records across three functions match the exact path captured by the compiler.
The remaining raw refusals are 20 unsupported and nine edition errors.
Production refuses all 45: 17 unsupported, nine edition and 19 intent errors.
No temporary path is normalized. The fresh final qualification below confirms
these measurements independently.

## Host input drift and fresh qualification

The first qualification cohort, retained in `qualification/`, stopped in
`make check` after 620.1 seconds at the Linux loader's installed-input hash
guard. No language or native admission check failed. Host libc6 had moved from
`2.39-0ubuntu8.9` to `2.39-0ubuntu8.12` on November 15; both the dynamic linker
and libc changed, while `/usr/bin/true` still matched its reviewed hash.

The complete original three inputs remain available from the prior Linux
checkpoint. Their hashes and reports agree with the explicit review in commit
`0419f1e8c277ce6f822663995f4348adc9c179f0`. Copies are retained separately in
`linux-fixtures/`, with provenance in `linux-input-provenance.json`. No host
package or library was modified, and no expected hash was changed.

The host artifact tool now accepts explicit `ANOMALOUS_LINUX_LOADER_PROGRAM`
and `ANOMALOUS_LINUX_LOADER_INTERPRETER` paths alongside its existing libc
selector. Installed paths remain defaults. Selection never bypasses bounded
snapshot reads or fixed hashes, and invalid configured input never falls back.
Guest paths, source bytes, loader geometry and syscall policy remain unchanged.
An independent review finds no defect in this narrow configuration change.

Because this is a source change, the original cohort receives no final
qualification credit. A fresh `qualification-final/` cohort reruns all 50 jobs
on a new complete source freeze, with explicit retained-input paths and input
hash checks before and after the run. The fresh 726-file manifest has SHA-256
`f6b2f4b66fbb7dd81c2042805fa672c536036bae903fc93e8541783534976b1c`. Earlier evidence and the failed log stay
intact. The language evidence below comes entirely from the fresh cohort.

All 13 explicit-input checks pass: one real configured build reproduces 18
retained artifacts byte-for-byte, and 12 changed/empty/missing selections
refuse without changing artifact bytes, modification times or inodes. Fixture
and host files remain unchanged. The separate selector receipt has SHA-256
`72af82f76c19334d2ee30adb210fe58d30405284c7fe9a65503c2136e8feb899`.
These are tooling checks, not additional language or guest execution coverage.

## Final language evidence

All eleven language proof suites pass unchanged against the frozen source:
126, 77, 61, 27, 55, 87, 103, 115, 105, 149 and 111 cases, totaling 1,016.
The final source-debug breakdown matches the revised development measurement.
All 260 oracle checks pass: one text, ten array, 19 callable, 42 record,
43 global, 73 enum and 72 source-debug checks. These are guard checks and
provide no additional corpus-match credit. Both freestanding conformance
feature exclusions pass.

The unmodified runner retains 97 compiler calls, 38 roots, 45 interpreter
invocations, all auxiliary obligations and its telemetry exit-only exception.
All fifteen prior raw matches remain. Independent audits confirm every proof,
the exact raw and production lanes, all source/tool/input identities and the
original checkout's preserved tracked diff and seven unrelated files.

The final new target is auxiliary invocation 4, captured as
`source-debug-proof/upstream-capture/0054/input-0.nil`. It is 3,511 bytes with
SHA-256 `1fbb14ea5d3f47e324c4c61519a7718bf5e7824657cb72d4bc616e06c844c7fd`.
Its source path is exactly `/tmp/tmpc8nc9r2j/source-debug.matrix`, drawn from the
captured compiler input; it was never rewritten or normalized. The reference
and candidate agree on all 23 records and the full signed-i64 zero result.
The 28 instructions, one console request and exact `14\n` output are checked
independently of the process's low-byte exit status.

Final source-debug proof summary SHA-256:
`00646c3e530965c40c862d46513b8003286712ee03cdab5521fcf41e7f0a1776`.
Independent retained-evidence summary SHA-256:
`021569ac8c8cc43e57174561738a9a7e2d84222c4a269f14fb629c206d78df76`.
Target receipt SHA-256:
`18d879f680eff086c3ffc6f58ea4162322401dc3fdda04ee51bd1b9647228ae6`.
Raw candidate SHA-256:
`51b103d1a767999ce40d2c3991fe3d859148986da75c4143e176829049711421`.
Production candidate SHA-256:
`73686288f5c50682452e9e07ebe3659619dce0ba282368e963253a851e71a396`.
Independent C++ oracle binary SHA-256:
`81b053aca37eea0b062a351cb25fd98c0d3857f3cb82790a8cdf2f3ec457ab29`.

The final target review has SHA-256
`f4022fd8af1a2ea5815da71a7cb1f1386696e7fc10bbb25737721090f890f1f0`.
The next-slice audit pins all 29 remaining refused editions and has SHA-256
`ac47c4f494518bff8ef6d040f5732e16dacfa489280d1216a6f804e978918549`.
Independent parser, executor/probe and proof reviews find no open defects.
The independent evidence verifier additionally rejects 21 deliberately mutated
metadata/result receipts.

## Final system qualification

All **50 fresh final jobs pass on their first attempt**, totaling 1,674.1
seconds of serialized job time; `make check` accounts for 780.1 seconds.
Every required AGENTS check and smoke target passes, alongside all eleven
language proofs, seven oracle-guard jobs, feature exclusions, native admission
and delivery, manifest qualification and Linux-loader entry. No memory or
timeout guard fired. The earlier 620.1-second failed check is retained in its
separate incomplete cohort and contributes no final qualification credit.

The independent aggregate verifies all final commands, results and logs,
source/tool/input identities, complete corpus lanes and original-checkout
preservation. Host counts are flow 133/159 and executor 91/100
(default/conformance), kernel 648/731 (default/all features), xtask 425,
runtime 37 and manifest 18. Three flow and one executor compile-fail witness
tests pass. The eleven Python suites pass 6/5/4/5/5/6/7/7/7/7/9 selftests.

The canonical aggregate `qualification-final/summary.json` has SHA-256
`19428e9740258a66cb5d1f406865bbab4eeda5171030bca891ab514a332e4e15`.
All 726 frozen source files and nineteen prior proof/oracle sources remain
unchanged. The original checkout's tracked diff and seven unrelated files are
preserved. Documentation links and the source freeze are checked again before
the completed checkpoint is committed.

## Native continuity

Fresh TCG native admission at 96 and 320 MiB preserves four admissions, six
refusals, twelve reads, one fixed loader marker and zero NIL execution, with
complete cleanup. The independent verifier passes all 8,614 mutation checks.
Its retained JSON has SHA-256
`edb06f7950cf8d7212c55ae7686686d1ad2f42af0cb86310d0d1d5f2f9db9f3f`.
Boot image, kernel, client ELF, client RX span and payload object retain their
exact historical identities. This is fresh TCG evidence and artifact continuity
with the earlier AMD KVM proof, not a new KVM run. No boot implementation changed,
and no new VirtualBox run is claimed.

Native program delivery also passes at both RAM sizes with its prior boot-image
SHA-256 `00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b`.
Linux-loader entry retains twelve requests and eleven returns at both sizes,
stopping at the unanswered sequence-12
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
The selected input copies preserve that exact behavior. No Linux application
exit or new syscall support is established.

## Remaining gates

Textual source metadata does not admit the separate binary NIL v5 artifact.
The next runtime expansion remains explicitly typed bool/u8/f64 values and
bounded scalar casts, including complete validation of unused helpers. Full
corpus qualification, native NIL execution, production intent admission and
general Linux application compatibility remain unqualified.
