---
title: "Host Conformance Boundary Log"
date: 2026-11-13
status: qualified-host-boundary
---

# Host conformance boundary

[ADR 0107](../../docs/adr/0107-separate-host-conformance-from-production-admission.md)
separates language validation from production admission while preserving both
as explicit obligations. The implementation starts from `d499af3` in the
isolated `codex/nil-conformance-boundary` worktree. The original checkout and its
unrelated work are preserved.

## Implementation

The `host_conformance` feature is disabled by default. Its raw scalar validator
returns an immutable `checked_edition` with exact borrowed bytes, SHA-256 identity,
optional authored-main intent and derived/effective service masks. Production
encode/admit entry points select the private production purpose explicitly;
there is no policy stored in reusable workspace state, raw-to-admitted
conversion, alternate envelope or public unchecked plan execution entry.

Only absence of main intent differs. An annotation present in the input retains
all shape, placement, uniqueness, text and size checks. The shared parser still
validates complete syntax, references, CFG joins and local initialization, and
accounts for dead and unused effects before applying an explicit known ceiling.
The raw feature has a compile-time rejection for freestanding targets.

Both concrete executor entries share a private iterative machine and console
mediation. Execution retains the exact edition identity, bounds all work and
storage, clears storage at each boundary, and preserves acknowledged output
before a later fault. No native task authority is granted by a host witness.

## Retained evidence

Artifacts are outside the repository at
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-conformance-2026-11-13`.
The pinned reference checkout remains revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. All six earlier proof/oracle sources,
compiler/interpreter binaries and both linked reference archives retain their
pins. The existing scalar C++ oracle already accepts absent intent; it is
reused unchanged against the real pinned reference VM.

The final implementation freeze includes **678 non-Markdown source files**.
`qualification/source-sha256.json` has SHA-256
`9b39935a35106d571547219e1a228169b58459238c871ea1252e238b846fc5d0`.
Default-feature profile-1/2/3 candidates and the opt-in raw candidate are built
serially and copied out of the shared mutable Cargo target before proofs run.
The guarded runner owns only its own process groups, with one build worker,
4 GiB memory-high, 6 GiB memory-max, 512 MiB swap-max and a 1 GiB available-host-
memory cutoff. Existing user processes and VM sessions are not stopped.

## Preliminary checks and retained failures

The development runner first failed before any job started because its job
inventory had not yet been written; its traceback is retained. The first flow
test build then found `E0502` in the new workspace-reuse test: reusing a validation
workspace also retained the prior input buffer's immutable borrow. A distinct
output buffer preserves that test's policy-reuse assertion without violating
input ownership. The analogous executor parity test uses per-edition storage
while reusing it across the raw and admitted runs of that edition.

The subsequent focused flow tests, executor tests and host Clippy pass. The
first new proof passes 27 boundary cases and the complete upstream capture.
The final harness then pins the observed four raw matches and refusal inventory,
with a selftest that rejects an all-refused regression; final qualification runs
that stronger harness against freshly captured corpus artifacts.

## Qualification

All **29 final jobs pass on their first attempts**, with no guard interruption.
The exact inventory is retained in `qualification/jobs.json`: three reference
builds, immutable candidate builds, the original three differential proofs, the
new conformance proof, explicit freestanding feature exclusion, `make check`,
every repository-required smoke/exclusion target, manifest proof and fresh
native admission/delivery/Linux regressions. The serial jobs total
21.4 minutes.

The independent final audit confirms all 678 frozen sources unchanged, complete
raw process evidence for all four proofs, expected feature exclusion, exact
native identities and preservation of the original checkout's tracked diff and
seven unrelated files. `qualification/summary.json` has SHA-256
`6b974c90b01f5dd654037bdfbe0f5f524080a35c891a065f3cec61e464d85bc8`. Its verifier imports independent retained-evidence
checkers, not the new proof harness.

Host results include 648 default and 731 all-feature kernel tests, 425 tooling,
37 native-runtime, 18 manifest, 37 default executor and 38 default flow tests.
The opt-in suites pass 50 flow and 41 executor tests, plus three flow and one
executor compile-fail doctests. The four Python harnesses pass 6/5/4/5 tests.
Warning-denied host/freestanding Clippy and formatting/artifact checks pass.

Fresh native delivery preserves image SHA-256
`00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b`.
Linux still records 12 requests and 11 returns, stopping at the unanswered
sequence-12 `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)` at both
96/320 MiB. This checkpoint adds no Linux application execution or exit.

## Differential and native receipts

The final bounded proofs pass unchanged: 126 profile-1, 77 profile-2 and
61 profile-3 cases. The new proof passes **27 boundary cases**: eight exact
semantic matches, one matching division-by-zero fault with prior output retained,
one finite-reference fuel-policy contrast and 17 validation refusals. All 27
also receive an independent production attempt: two annotated successes and
25 expected admission refusals. Ten finite C++ reference comparisons complete;
no timeout counts as a match. Five Python harness selftests pass.

The independent checker reads retained raw process outputs, commands, statuses,
compiler artifacts and hashes without importing the new proof harness. It
verifies both corpus lanes, the complete upstream capture, exact expected
boundary outcomes and the positive-coverage regression contract. Each interpreter
input also matches a preceding retained complete compiler artifact. Its final
receipt is `qualification/independent-conformance-summary.json`, SHA-256
`fadc46c41b2cd0ef8ec4b7c0f702bfd5fb9635104c02e846601a6ca098518058`.
The raw candidate SHA-256 is
`9c75972a147cc35b68547c7adb3dfc33db4237db85406e9a9d8a433c6483c0b3`;
the default-feature production profile-3 candidate SHA-256 is
`838ee548d9a5c0b715c43aebdcb6349082a6364bac64383de225a9a1ff84edd8`.

Fresh TCG native admission passes at 96/320 MiB: four admissions, six refusals,
12 edition reads, one fixed loader marker and zero NIL executions, with complete
cleanup. All five admission artifacts match the preceding scalar checkpoint
byte-for-byte: boot image, kernel ELF, client ELF, client RX bytes and embedded
payload object. All 11 sources from the prior emitted-code review are unchanged.
The independent native checker rejects 8,614 transcript/report mutations.
Historical AMD KVM evidence is reused only for those identical artifacts;
there is no fresh KVM or VirtualBox execution claim.

## Coverage interpretation and next work

The initial raw run matches four of 45 interpreter invocations: three auxiliary
source-extension selections of the same 306-byte edition and the 298-byte
`hello.matrix` root edition. This is **one of 38 root cases** and two distinct
edition contents, not four newly supported root programs. Their original
reference process exit statuses and console bytes match; full-i64 result
comparisons are separately exercised by the boundary oracle.

The remaining 41 raw invocations refuse: 32 unsupported forms and nine edition
limits/formats. The nine are eight textual editions above 64 KiB and one binary
edition. Production admission independently refuses all 45 unchanged editions:
28 unsupported, nine edition and eight intent first refusals. No refusal receives
semantic coverage credit. All 45 lack the required supported authored-main
intent, including editions whose earlier refusal masks that later policy gate.

The corpus runner and complete compiler/interpreter artifacts are unchanged.
Its 97 compiler calls, 38 roots, 45 interpreter invocations, schema twins,
diagnostics, imports and existing single telemetry exit-only exception remain
part of the gate. The new exact raw-coverage contract prevents this checkpoint
from passing if its four known matches become refusals. Full candidate
qualification and native execution remain false.

The next small language increment is bounded computed text plus signed-i64
hexadecimal literals. `short-extension.mat` needs one `STR_CONCAT` returning
text from a helper; the scalar Ny/Matrix twins need text concatenation and three
hex constants. Those complete editions fit current structural limits. Computed
text needs execution-owned decoded storage that survives callee return, precise
alias/escape behavior, explicit exhaustion and clearing on every exit. Supporting
these forms needs a separate reviewed grammar/profile and measured corpus proof;
no prospective coverage is counted here.
