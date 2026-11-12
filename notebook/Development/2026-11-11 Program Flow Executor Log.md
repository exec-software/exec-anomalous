---
title: "Program Flow Executor — bounded functions, branches and loops"
date: 2026-11-11
status: bounded-host-proof
tags: [native-abi, program, interpreter, manifest, control-flow, corpus, qualification]
---

# Program flow executor

This checkpoint develops host-only profile-2 admission and execution for bounded
functions, branches, loops and recursive calls. Its accepted contract is
[ADR 0105](../../docs/adr/0105-validate-bounded-function-control-flow-before-host-execution.md).
The bounded 77-case proof, unchanged 126-case profile-1 regression and all
25 final repository qualification jobs pass. Profile 1 and the
native manifest loader retain their existing contracts; no NIL edition executes
natively in this slice.

## Isolated admission and plan

`anomalous-program-flow` owns the new parser, resolver, CFG validator and private
admission witness. The canonical envelope keeps version 1 and its existing
header layouts but requires profile 2 in both headers. Each profile refuses the
other. The witness borrows exact immutable edition bytes and a caller-owned
validated plan; a caller cannot supply an unchecked plan to execution.

All instructions are parsed and references resolved, including dead instructions
and unused functions. Their console effects contribute to the complete authority
union before the ceiling is checked. Unknown dead operations and malformed calls
remain refusals. Globally unique labels cannot collide with function names;
branch targets stay within their function. Locals and parameters cannot shadow
functions or console builtins and thereby redirect a nominally static call.

Every function is an independent CFG root. Its declared arguments start on the
operand stack in order; the program's own reversed stores bind named locals.
Joins require identical operand stacks. Definite local initialization is the
intersection of incoming paths: an uninitialized input loses a known kind,
while conflicting initialized kinds reject. Reachable loads require initialization
on all paths. Loop backedges use the same rules, without assuming the loop ran.

Only entry-reachable instructions receive stack/local dataflow validation. Dead
blocks still receive syntax, reference and authority checks; there is no claim
that their hypothetical operand states are valid. This admits intact compiler
early-return layouts containing dead jumps, labels and the exact generated
nonvoid-fallthrough marker. Reaching that marker produces a runtime error.

## Storage and runtime limits

The artifact is bounded to 64 functions, 128 constants, 4,096 instructions,
eight parameters per function, 128 locals per function, 256 basic blocks and
1,048,576 validation instruction transfers. The existing 65,536-byte edition,
512-byte plain intent and bounded identifier limits remain explicit.

Execution uses 256 total operand values, at most 16 frames including `main`,
128 locals per frame and 65,536 dispatched instructions. It uses an explicit
frame array rather than Rust recursion. Each frame preserves the caller's stack
prefix and resume position. Calls and returns do not implicitly narrow integers.
Helpers may exchange integer/text values or return void; `main` has no arguments
and returns an integer. Text values borrow literal storage.

Console limits remain 4,096 bytes per request and 65,536 acknowledged bytes per
run. Complete payload checks precede publication. Earlier output remains visible
after a later refusal; attempted calls and acknowledged bytes are distinct, and
the executor never retries a refusing sink. Runtime workspace clearing does not
undo external effects.

The cores are `no_std`, unsafe-free and allocate no heap. Host tooling may box
the caller-owned workspaces; admission unit tests use explicit 8 MiB thread
stacks for fixed metadata storage. These are host test placements, not evidence
for native stack size or a new mapping profile. Integer conditions are the only
admitted branch conditions. Dollar temporaries, untyped arithmetic, hexadecimal
literals and text concatenation remain unsupported.

## Complete compiler fixtures

The six `.ny` sources and complete `.nil` outputs reside in
[`fixtures/flow`](../../crates/anomalous-program-executor/fixtures/flow/).
Both the final bounded proof and its independent verifier retain source and
artifact hashes for compiler revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. Each fixture is freshly compiled using
`--quiet --no-stdlib --no-bundle --no-context-registration`; the complete result
must match its checked-in `.nil` byte for byte.

The golden editions retain the compiler's trailing blank line. Do not normalize
their whitespace: those exact bytes are part of each pinned artifact identity.
Candidate and reference agree:

| Fixture | Edition bytes | Result | Exact output |
| --- | ---: | ---: | --- |
| `branches` | 2,375 | 32 | Empty |
| `calls` | 1,743 | 42 | Empty |
| `early_returns` | 1,350 | 0 | Empty |
| `recursion` | 1,174 | 120 | Empty |
| `text_void` | 1,566 | 0 | Hex `7965730a` (`yes` + LF) |
| `while` | 2,385 | 13 | Empty |

The fixtures exercise ordered arguments, nested local isolation, branch joins,
zero-iteration loops, continue/break, recursion, borrowed text, void calls and
unreachable compiler fallthrough. No instruction stripping or artifact rewriting
is part of the admission path.

| Complete edition | SHA-256 |
| --- | --- |
| `branches.nil` | `bf14aa9997013b1753e76f513ddfb4d1345d2b890085f6d3805777af65e23774` |
| `calls.nil` | `7e923aa1f9a4c8837d0067d2bf4c40136145b925bed4f7c6caa901601532cf06` |
| `early_returns.nil` | `cc3f6a37f70f1b46cdbecc63873032312739b25979aeab014c23896ee7866663` |
| `recursion.nil` | `7eb4040f8010338aaf5b4ee387df44d9ad00442653bc7f5442c78676a241150a` |
| `text_void.nil` | `11e9a40309ac76b5d85eeba2876d4f1001695441128ada790ed88255aa9b41f9` |
| `while.nil` | `5caccb39df831d7f125ba355ad3ac17e5dbbad717d345c4997fac5de992e2952` |

## Bounded differential proof

The new proof passes all **77 cases** with no failed case:

| Outcome | Cases | Evidence |
| --- | ---: | --- |
| Exact semantic match | 38 | Full signed integer result and exact output agree, including all six compiler fixtures |
| Matching runtime fault | 2 | Division by zero and reached generated fallthrough; exact reference diagnostics and retained prior output |
| Deliberate runtime-policy refusal | 4 | Finite reference completes; candidate refuses its smaller instruction, frame, aggregate-stack or output budget |
| Candidate-only bounded policy | 4 | Infinite branch/call cycles stop at limits; two excessive caller caps refuse before execution |
| Expected admission refusal | 29 | Malformed CFG/references/types, dead or unused forbidden effects, denied console authority and structural caps |

There are **44 finite reference comparisons**. The four candidate-only cases do
not execute in the reference, and a timeout is always a proof failure. Written
labels consume candidate fuel. Focused generated cases check exact dispatch
counts, including failed instructions and pre-dispatch exhaustion; compiler
fixture counts are checked against the bound while their results and effects
are compared independently.

Eighteen selected editions also refuse through the unchanged profile-1 probe.
These are raw NIL syntax checks. The flow crate's canonical-wire unit test
separately checks that each admission path rejects the other profile's package
tag. Neither check implies that profile 1 has been widened.

The original proof script and C++ reference source are unchanged. A fresh
profile-1 regression passes all **126 cases**: 100 exact semantic matches,
20 matching runtime faults and six deliberate output-policy refusals. Its
separate result remains required; the new 77 cases do not replace it.

## Full corpus gate

Each proof separately runs the pinned, unmodified upstream runner and retains
all **97 compiler calls, 38 root fixtures and 45 interpreter invocations**.
The reference passes all obligations, including three identical schema
comparisons, 13 diagnostics, 26 foreign-keyword checks, eight standard-library
fixtures and the existing one clock-dependent exit-only case. Capture wrappers
forward the original arguments, outputs and process status; six temporary include
dependency files are retained in each proof.

Every one of the 45 profile-2 candidate attempts refuses admission: 28 report
`manifest_unsupported`, nine `manifest_edition`, and eight `manifest_intent`.
These are the first reported refusal reasons, not a complete inventory of all
missing features. Only four retained corpus editions carry an intent marker;
those helper/type annotations do not supply the required authored `main`
intent. All 45 corpus inputs lack that required entry contract. The nine
edition refusals include eight textual outputs beyond the 64 KiB profile bound
and one binary edition.
The separate profile-1 ledger also has 45 admission refusals, with its original
24 unsupported, nine edition, seven syntax and five intent failures.

No corpus source, emitted edition, expected output or runner selection was
changed to improve these counts. Both summaries retain
`candidate_qualified: false` and `native_execution: false`. Safe refusal is unmet
compatibility coverage, never an execution pass.

## Retained verification and final qualification

The qualification source manifest freezes **651 non-Markdown files**. The
bounded proof and original regression use separate copied probe binaries, so
later Cargo builds cannot replace the measured inputs at their paths.

| Retained identity | SHA-256 |
| --- | --- |
| Frozen source manifest | `cd912f275eb521bed533370a087269eda7b701561fe0a615fb639ee3d0a422ad` |
| Final qualification summary | `ca1b10b9882dff05e898dca1bf55aead3ece6366148e0929c57376de86a4653a` |
| Profile-2 candidate | `22714717b6bed81ec65c238eced1e64810b3cd57e5d217fb27c93a67d1157cf9` |
| Profile-1 candidate | `2e4eb6ed4ead235b68d8a47fe59c4e0d93570aa909ad32a7fc46ed81643620f8` |
| Flow C++ reference | `77d7e5e30af34272dac3b0c48aa72adcb8d90864d393187c52d4837b77a32001` |
| Full corpus/stdlib input manifest | `4328eb0a154f6f5443fe85065cb6b132c71a6b1f1c2e13993ae10ffe738006a1` |

Independent retained-evidence verification passes for both proofs. It checks
raw process stdout/stderr/status, exact editions and compiler sources, tools and
linked reference archives, all outcome counts, cross-profile raw-edition
refusals, retained include dependencies and the unchanged original worktree
preservation baseline. These checks read the evidence without importing the
Rust implementation or treating a summary verdict as sufficient proof.

Evidence resides in `../program-flow-2026-11-11/qualification` relative to the
isolated checkout. The separate `proof` and `profile1-proof` directories retain
their complete captures. `independent-flow-summary.json` and
`independent-execution-summary.json` record the independent results; their
candidate paths identify the retained binaries. Development runs remain under
the sibling `development` directory.

The fresh native admission regression passes under TCG at 96/320 MiB. All five
compared native artifacts are byte-identical to the prior checkpoint. Each boot
still records four admissions, six semantic refusals, twelve reads, one fixed
loader console write and zero edition executions. Missing, wrong-size and
corrupted-source provisioning refusals also pass. Independent transcript
verification rejects all 8,614 mutations in its oracle self-test. Prior KVM
evidence belongs to the preceding checkpoint; this slice claims no new KVM run.

All **25 final qualification jobs passed** against the frozen 651-file source
manifest. Every command required by `AGENTS.md` passed, together with the two
reference builds, retained candidate build, both differential proofs, host
manifest proof, fresh native admission and delivery regressions, and Linux
loader-entry regression. There were no failed final attempts or memory-guard
terminations. The independent final verifier recomputes source identities,
checks the exact job inventory and logs, re-verifies both raw proof captures,
and checks native artifact continuity and original-checkout preservation.

| Host suite | Passed |
| --- | ---: |
| Kernel, default / all features | 648 / 731 |
| xtask | 425 |
| Native runtime | 37 |
| Program manifest | 18 |
| Program executor | 31 |
| Program flow admission | 22 |
| Original / flow Python proof self-tests | 6 / 5 |

The native admission boot image retains SHA-256
`20df2f2f40235d08e7fb7fbf2b19f5dd78c4406af5a94bec18066c2a0a0b2464`.
Its 18,666 RX bytes retain
`6508bde6dd4e9cb87afda6764d29c0937fa37c8e7eab8e8b1633ed67d0a14adf`.
Kernel ELF, client ELF and payload object also match the preceding checkpoint
byte for byte, as do all eleven sources in its emitted-code review. That prior
review and historical AMD KVM evidence remain applicable to those identical
artifacts; neither is presented as a fresh KVM run or native NIL execution.
The native edition-delivery image remains byte-identical as well.

Linux still issues twelve requests, returns from eleven and stops at the
unanswered sequence-12 `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`
at both tested memory sizes. No Linux application exit is observed. This host
checkpoint makes no new VirtualBox or native interpreter claim.

Two preliminary failures are retained separately: test fixture lifetime errors
were fixed by giving each mutated image its own validation workspace, and a
Clippy collapsible-condition finding was resolved before the final source
freeze. Their failed logs remain under `development`; the earlier binaries
are not substituted for the retained final candidates.

The original Anomalous tracked binary diff and seven unrelated files still
match the preservation baseline. The qualified NIL checkout remains clean at
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. No installed SDK or stable public
ABI was changed.

## Reproduction

Use the qualified NIL checkout and its already-built compiler, interpreter and
archives. From the Anomalous checkout, create a new evidence directory, build
both probes and copy them before subsequent Cargo commands. Keep those copied
executables and reference binaries as read-only inputs throughout both proofs:

```sh
nil_reference_root=/path/to/qualified/nil-context-registration
flow_evidence_dir="$(pwd)/build/program-flow-proof-001"
mkdir -p build
mkdir "$flow_evidence_dir"
mkdir "$flow_evidence_dir/bin"

cargo build -p anomalous-program-executor \
  --example program_flow_probe --example program_execution_probe
cp target/debug/examples/program_flow_probe "$flow_evidence_dir/bin/flow-candidate"
cp target/debug/examples/program_execution_probe "$flow_evidence_dir/bin/profile1-candidate"

for reference_profile in execution flow; do
  c++ -std=c++17 -O2 -I"$nil_reference_root/nilcompiler/src" \
    "scripts/program_${reference_profile}_reference.cpp" \
    "$nil_reference_root/build/context-registration/lib/libnilcompiler_core.a" \
    "$nil_reference_root/build/context-registration/lib/libnil.a" \
    -ldl -pthread -o "$flow_evidence_dir/bin/${reference_profile}-reference"
done
chmod a-w "$flow_evidence_dir"/bin/*
sha256sum "$flow_evidence_dir"/bin/* > "$flow_evidence_dir/binaries.sha256"

python3 scripts/program_execution_proof.py --self-test
python3 scripts/program_flow_proof.py --self-test
python3 scripts/program_execution_proof.py \
  --candidate "$flow_evidence_dir/bin/profile1-candidate" \
  --reference "$flow_evidence_dir/bin/execution-reference" \
  --compiler "$nil_reference_root/build/context-registration/bin/nilcompiler" \
  --root "$nil_reference_root" \
  --output "$flow_evidence_dir/profile1-proof"
python3 scripts/program_flow_proof.py \
  --candidate "$flow_evidence_dir/bin/flow-candidate" \
  --profile1-candidate "$flow_evidence_dir/bin/profile1-candidate" \
  --reference "$flow_evidence_dir/bin/flow-reference" \
  --compiler "$nil_reference_root/build/context-registration/bin/nilcompiler" \
  --root "$nil_reference_root" \
  --output "$flow_evidence_dir/flow-proof"
```

Choose another new directory for another run; neither proof overwrites earlier
evidence. Tool hashes identify each measured run; the table above identifies
this retained qualification, rather than promising identical debug binaries
from every host toolchain. `make check` covers crate tests, proof self-tests and
lint checks. The external-toolchain differential and complete corpus proofs are
additional explicit runs.

## Remaining gates

The full interpreter acceptance gate remains closed: 38 root fixtures, all
45 captured interpreter invocations, three schema comparisons and the pinned
runner's compiler/diagnostic obligations remain required. Safe refusal of an
unsupported fixture is not an execution pass. Wider types, aggregates, faults
and service families remain separate work. Native task-bound effect mediation,
execution and interpreter geometry are not qualified by this host checkpoint.

The next bounded compiler-form increment can cover generated `$ny.temp.*`
locals and operand-less integer `ADD`, using complete new compiler outputs for
range loops and short-circuit expressions. That work must retain local-name
bounds, resolved call names, typed joins and runtime limits. It cannot by itself
qualify the unchanged corpus: raw semantic-conformance testing and production
manifest admission need an explicit architectural distinction, without
injecting intent into corpus artifacts or waiving production admission policy.
