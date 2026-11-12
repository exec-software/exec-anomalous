---
title: "Compiler Scalar Flow — generated locals and integer ranges"
date: 2026-11-12
status: bounded-host-proof
tags: [native-abi, program, interpreter, manifest, control-flow, corpus, qualification]
---

# Compiler scalar flow

This host-only checkpoint adds explicit profile 3 for compiler-generated scalar
temporary locals and operand-less integer `ADD`. Its accepted contract is
[ADR 0106](../../docs/adr/0106-admit-bounded-compiler-scalar-control-flow.md).
The frozen differential proof passes all 61 cases, and all three profile proofs
pass independent retained-evidence verification. All 27 final repository
qualification jobs pass. The original profile-1 and profile-2 entry points
retain their existing contracts; native NIL execution and full interpreter
qualification remain gated.

## Narrow admission extension

`anomalous_program_flow::scalar::encode` and `scalar::admit` require profile 3
in both existing envelope headers. Existing entry points still require their
own profile tags. A private closed policy shares parsing, canonical encoding,
resolution and CFG validation. The admitted witness exposes its profile but
cannot be constructed or modified by callers. Execution consumes the same
immutable validated plan through the existing iterative executor.

Only `LOAD_LOCAL` and `STORE_LOCAL` gain generated names of the form
`$ny.temp.<simple_ascii_identifier>.<canonical_decimal>`. The entire name remains
bounded to 128 bytes, and generated and ordinary locals share the same 128 slots.
Exact spelling determines identity. Counter suffixes are not parsed into machine
integers, and leading zeros are refused except for `0` itself. Generated names
remain invalid for functions, parameters, constants, labels and call targets.
Dotted prefixes used elsewhere in the compiler are outside this scalar subset.

Only operand-less `ADD` gains admission; reachable CFG analysis must establish
two integer operands. It normalizes to the existing wrapping `ADD i64` operation.
Floating-point and array producers remain unsupported, and text addition refuses
admission. All other untyped arithmetic remains unsupported. Dead instructions
still receive syntax, reference and authority checks without invented operand
states. The complete artifact determines authority, including unused functions.

No new runtime storage, allocator, service, native profile or kernel feature is
introduced. Validation and runtime limits remain those of ADR 0105. The host
tests use explicit 8 MiB thread stacks for fixed validation metadata; this does
not establish native stack geometry.

## Pinned compiler semantics

The compiler checkout remains at
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. Its `codegen.cpp` emits generated
names by concatenating `$ny.temp.`, a compiler-selected prefix and a decimal
per-function counter. Scalar prefixes are simple identifiers; that is not the
complete grammar of all compiler-generated names.

Range bounds execute once in start-then-end order. The hidden index uses signed
64-bit comparison against an exclusive endpoint and an operand-less increment.
The compiler explicitly casts the visible loop variable to 32 bits. `continue`
reaches the increment, and `break` bypasses it. An ordinary ascending range with
a representable exclusive endpoint does not overflow its hidden index; wrapping
arithmetic is tested separately with complete authored NIL editions.

Short-circuit false `&&` paths produce zero and true `||` paths produce one.
When the right-hand expression runs, its result is retained. Output and arithmetic
faults in skipped calls must remain unobserved. The independent reference oracle
uses the pinned VM, preserving its exact integer result and output bytes.

## Complete compiler fixtures

Eight authored sources and their complete compiler outputs live in
[`fixtures/scalar-flow`](../../crates/anomalous-program-executor/fixtures/scalar-flow/).
They compile with `--quiet --no-stdlib --no-bundle --no-context-registration`.
No helper stripping, injected intent or output rewriting is used. The `.nil`
files retain the compiler's trailing blank line as part of their exact identity.

| Fixture | Reference result | Exact console output |
| --- | ---: | --- |
| `zero_ranges` | 7 | Empty |
| `nested_ranges` | 189 | Empty |
| `break_continue` | 13 | `0`, `1`, `3`, `4`, `5`, each followed by LF |
| `bounds_once` | 9 | `2` then `5`, each followed by LF |
| `short_circuit` | 15 | `33` then `44`, each followed by LF |
| `scratch_joins` | 430 | Empty |
| `range_widths` | -3 | `2147483647`, `-2147483648`, `-2`, each followed by LF |
| `fuel` | 10000 | Empty; candidate must refuse at 65,536 dispatched instructions |

| Complete edition | Bytes | SHA-256 |
| --- | ---: | --- |
| `bounds_once.nil` | 1,826 | `517a4e0cdfa225ff79e6782b93d28a9a1901831ddfc7f2c16827b1f98da1cce2` |
| `break_continue.nil` | 1,800 | `1ff7284b4c4a62b27ec1f7c8a8448c2571e622f79a80dd8088fc8bc581caf03f` |
| `fuel.nil` | 1,255 | `870f0910c27b41c6e193aec3c8e587318bb91e3ded64e07b3fe692314ce471fd` |
| `nested_ranges.nil` | 2,110 | `36632a21a36b00b7623ab08cf450f49b30287109c8418e88d3f4abcd03b53b36` |
| `range_widths.nil` | 2,433 | `d2e3601d8e748c2a6443bc36c2dfb66acbdf6f418b2a666a25d8c9385e1b5012` |
| `scratch_joins.nil` | 4,278 | `98faa97f55cecbbc641eeace02569502860d64452aaa8e2f53633d74770f7cb2` |
| `short_circuit.nil` | 5,966 | `b73bed9dc490970614e4a04010f2eb519fb45884de90f91e7309a808f758de8a` |
| `zero_ranges.nil` | 4,035 | `826a01f5ff08dfb5500322e2206500451743494d2b36f06139fd74783ce0e32d` |

All eight development compiler and reference runs pass. The candidate's focused
tests pass 38 admission tests and 37 executor tests, including the existing
profile-2 cases, generated-local frame isolation, integer wrapping, exact compiler
results, service failure accounting and fuel exhaustion. Host and freestanding
lint checks pass. These preliminary checks do not replace final qualification.

## Differential proof and corpus evidence

The frozen profile-3 proof passes **61 cases**, with no failed case:

| Outcome | Cases | Evidence |
| --- | ---: | --- |
| Exact semantic match | 27 | Full signed results and exact output agree, including seven complete compiler fixtures |
| Deliberate runtime-policy refusal | 1 | The finite `fuel` fixture returns 10000 in the reference; the candidate stops after 65,536 instructions with no output |
| Expected admission refusal | 33 | Malformed or misplaced generated names, name/local limits, incomplete joins, noninteger `ADD`, denied authority and unsupported instructions |

All **28 admitted editions** also refuse through the unchanged profile-2 probe.
These are raw-edition syntax checks; profile-tag separation is checked by the
canonical-envelope unit tests. The proof does not count a timeout as a match.
Generated cases check exact dispatch accounting, wrapping at both signed 64-bit
limits, precision above 2^53, and the absence of implicit 32-bit narrowing.
Temporary-local tests cover exact 128-byte names, 128 shared local slots, frame
isolation, authored-name separation and definite initialization at joins.

The new harness's four self-tests pass. It reuses earlier capture and comparison
helpers through scoped bindings that are restored afterward. The original
126-case and 77-case scripts and reference sources remain unchanged and hash
pinned. Both separate frozen regression runs pass; the new 61 cases do not
replace either earlier suite. Compiler, interpreter and both reference archives
are pinned before the proof and rechecked afterward, as are complete fixture
inputs and retained copies.

The frozen proof freshly runs the unmodified upstream runner: **97 compiler
calls, 38 root fixtures and 45 interpreter invocations**, with complete inventory
and a passing reference result. All 45 profile-3 candidate attempts refuse
admission: 28 report `manifest_unsupported`, nine `manifest_edition`, and eight
`manifest_intent`. These are first-refusal diagnostics, not a complete inventory
of missing language support. Only four captured editions contain an intent
marker; those helper/type annotations do not supply the required authored
`main` intent. All 45 corpus inputs lack that entry contract. The nine edition
refusals include eight textual outputs above 64 KiB and one binary edition.
No corpus edition or expected result was rewritten to improve coverage.
The proof retains `candidate_qualified: false` and `native_execution: false`.

All three frozen profile proofs pass their independent read-only evidence
verifiers. The profile-3 verifier checks all 61 outcomes, 28 finite reference
comparisons, 28 raw-edition profile-2 refusals, process outputs and statuses,
exact fixture/tool hashes, the complete corpus ledger and six retained temporary
include dependencies. It also confirms the original tracked diff and seven
unrelated files still match their preservation baseline. The separate earlier
proofs retain their original 126-case and 77-case classifications.

The measured profile-3 candidate has SHA-256
`0da4a7e0eafcf9bd8ed8c60d341a66380a6cfd8be48c909a99e79dd1bf27914b`.
Its retained path and independent result are recorded in
`qualification/independent-scalar-flow-summary.json`. The three evidence trees
are `qualification/profile1-proof`, `qualification/profile2-proof` and
`qualification/proof` beneath the dated evidence directory.

The fresh native admission regression passes under TCG at 96/320 MiB; all five
compared native artifacts are byte-identical to the previous checkpoint.
Independent transcript verification passes, and its mutation self-test rejects
all 8,614 altered transcripts. The loader still performs admission without NIL
execution. KVM evidence remains historical from the earlier checkpoint; no new
KVM run is claimed here.

## Reproduction

Use the qualified compiler checkout and its existing compiler, interpreter and
static archives. From the Anomalous checkout, the following creates a new
evidence directory, copies all three probes and builds all three reference
drivers. The six executables become read-only inputs before the proofs run, so
later Cargo builds cannot replace their measured paths:

```sh
(
set -eu
nil_reference_root=/path/to/qualified/nil-context-registration
scalar_evidence_dir="$(pwd)/build/program-scalar-flow-proof-001"
mkdir -p build
mkdir "$scalar_evidence_dir"
mkdir "$scalar_evidence_dir/bin"
cargo build -p anomalous-program-executor \
  --example program_execution_probe --example program_flow_probe \
  --example program_scalar_flow_probe
for scalar_profile in execution flow scalar_flow; do
  cp "target/debug/examples/program_${scalar_profile}_probe" \
    "$scalar_evidence_dir/bin/${scalar_profile}-candidate"
  c++ -std=c++17 -O2 -I"$nil_reference_root/nilcompiler/src" \
    "scripts/program_${scalar_profile}_reference.cpp" \
    "$nil_reference_root/build/context-registration/lib/libnilcompiler_core.a" \
    "$nil_reference_root/build/context-registration/lib/libnil.a" \
    -ldl -pthread -o "$scalar_evidence_dir/bin/${scalar_profile}-reference"
done
chmod a-w "$scalar_evidence_dir"/bin/*
sha256sum "$scalar_evidence_dir"/bin/* > "$scalar_evidence_dir/binaries.sha256"

python3 scripts/program_execution_proof.py \
  --candidate "$scalar_evidence_dir/bin/execution-candidate" \
  --reference "$scalar_evidence_dir/bin/execution-reference" \
  --compiler "$nil_reference_root/build/context-registration/bin/nilcompiler" \
  --root "$nil_reference_root" --output "$scalar_evidence_dir/profile1-proof"
python3 scripts/program_flow_proof.py \
  --candidate "$scalar_evidence_dir/bin/flow-candidate" \
  --profile1-candidate "$scalar_evidence_dir/bin/execution-candidate" \
  --reference "$scalar_evidence_dir/bin/flow-reference" \
  --compiler "$nil_reference_root/build/context-registration/bin/nilcompiler" \
  --root "$nil_reference_root" --output "$scalar_evidence_dir/profile2-proof"
python3 scripts/program_scalar_flow_proof.py \
  --candidate "$scalar_evidence_dir/bin/scalar_flow-candidate" \
  --profile2-candidate "$scalar_evidence_dir/bin/flow-candidate" \
  --reference "$scalar_evidence_dir/bin/scalar_flow-reference" \
  --compiler "$nil_reference_root/build/context-registration/bin/nilcompiler" \
  --root "$nil_reference_root" --output "$scalar_evidence_dir/profile3-proof"
)
```

Use another new directory for a later run; all three output directories must
initially be absent. Each proof captures its own complete corpus ledger.
`make check` includes crate tests, lint checks and proof self-tests. The three
external-toolchain proofs above remain additional explicit runs.

## Final qualification

The final run freezes **672 non-Markdown source files**, identified by source
manifest SHA-256
`96635c2cd4e923b91ffd9584ce5b818febf568cd36a03668f0fdacb1acbebc79`.
All **27 final qualification jobs passed on their first attempts**, with no
memory-guard termination. Every command required by `AGENTS.md` passed, together
with three reference builds, the retained three-probe build, all three
differential proofs, host manifest proof, fresh native admission and delivery
regressions, and Linux loader-entry regression. All 16 development jobs also
passed; their evidence remains separate from the frozen qualification.

| Host suite | Passed |
| --- | ---: |
| Kernel, default / all features | 648 / 731 |
| xtask | 425 |
| Native runtime | 37 |
| Program manifest | 18 |
| Program executor | 37 |
| Program flow admission | 38 |
| Profile-1 / profile-2 / profile-3 proof self-tests | 6 / 5 / 4 |

The independent final verifier checks the exact job inventory and raw logs,
recomputes source identity, re-verifies all three proof captures, and checks
native artifacts, the earlier emitted-code review and original preservation.
The final summary has SHA-256
`a1056e1eb5b4e3583059f951be719aeef22df3c16f3a1ccc453bd5bec05d60ea`.

The native admission boot image retains SHA-256
`20df2f2f40235d08e7fb7fbf2b19f5dd78c4406af5a94bec18066c2a0a0b2464`;
its 18,666 RX bytes retain
`6508bde6dd4e9cb87afda6764d29c0937fa37c8e7eab8e8b1633ed67d0a14adf`.
Kernel ELF, client ELF and payload object are also byte-identical to both prior
native checkpoints. All eleven sources used in the earlier emitted-code review
remain unchanged. The historical AMD KVM proof remains explicitly historical;
this slice claims only a fresh TCG native regression. No native NIL edition
executes. The native edition-delivery image is also unchanged.

Linux still issues twelve requests and returns from eleven before stopping at
the unanswered sequence-12 `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`
at both 96/320 MiB. No Linux application exit or new VirtualBox result is claimed.
The original Anomalous tracked binary diff and seven unrelated files still match
their preservation baseline. The qualified NIL checkout remains clean at the
pinned revision. No installed SDK or stable public ABI was changed.

Evidence is retained beside the isolated checkout under
`../program-scalar-flow-2026-11-12`, including per-process output, exact editions,
three full corpus ledgers, retained executables and independent verification.
The original 126-case and 77-case proofs remain separate results. Candidate
corpus refusals remain unmet coverage. A later architectural decision must
distinguish raw language-conformance testing from production manifest admission
without altering the corpus or waiving the production intent contract. Full
interpreter qualification and native execution remain gated.
