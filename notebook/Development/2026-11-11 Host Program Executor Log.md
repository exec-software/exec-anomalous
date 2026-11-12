---
title: "Host Program Executor — bounded semantics before native execution"
date: 2026-11-11
status: bounded-host-proof
tags: [native-abi, program, interpreter, manifest, corpus, qualification]
---

# Host program executor

This checkpoint develops an experimental host-only execution core for the
already admitted NIL profile 1. Its contract is accepted in
[ADR 0104](../../docs/adr/0104-develop-a-bounded-host-executor-behind-the-corpus-gate.md).
The bounded host execution proof is implemented and passing. This slice adds no
kernel feature, native vessel, mapping profile, startup descriptor, gate or
report format. Shared tokenization rebuilds the existing native admission
loader; its regression evidence is recorded below. The loader remains an
admission proof with zero NIL editions executed.

## Why this scope

The accepted interpreter direction selects a Rust implementation, but its
Runtime Owner Reply makes the unmodified `matrix.execution` corpus a gate for
shipping the second VM. The current profile-1 parser accepts bounded
straight-line integer and console operations. It deliberately refuses much of
that corpus. Developing and comparing those first semantics on the host advances
the implementation without treating an unsupported corpus fixture as a pass.

The reference revision is compiler commit
`4dad0512dfd3fa4e351f2816196cb48956f5442e`, already used by the complete compiler
outputs in the native admission checkpoint. Differential execution must hold
exact edition bytes and retain reference-tool identities. The opt-in compiler
registration switch is not permission to strip helpers from corpus artifacts
or alter the unmodified corpus runner.

## Candidate contract

The `no_std`, unsafe-free core uses no heap. It accepts a borrowed
`admitted_edition` witness, retaining the checked bytes during dispatch and
using the granted workload surface for every effect. It shares instruction
decoding with admission. It executes `main`; unused functions still contribute
to the complete artifact's authority union.

Fixed limits are 256 stack values, 128 locals, 128 constants and at most 4,096
instruction steps, with 4,096 as the default step allowance. Output is bounded
to 4,096 bytes per publication and 65,536 acknowledged bytes total. Complete
console-payload validation precedes the sink call. Private stack state may
change before refusal; the failed machine is not resumable.

Console payloads must be nonempty UTF-8 with no controls except LF and TAB.
An empty non-newline write performs no publication; an empty newline-producing
operation still writes its newline. Validate complete formatted payloads before
the sink call. Never split, sanitize or truncate an invalid instruction's output.
The sink acknowledges the complete request or refuses it. A refusing sink may
already have effects; the executor does not promise atomic I/O or rollback.
It counts attempted calls separately from acknowledged bytes and does not retry.
Policy or budget rejection before a sink call publishes no bytes from that
instruction.

Explicit errors preserve the distinction between arithmetic failure, resource
limits, authority refusal, text-policy refusal and service failure. Earlier
successful output remains committed if a later instruction fails. The result
and evidence must retain that progress, without claiming whole-program rollback.

The native console's restrictions matter here: the admitted language can
describe string escapes that the service cannot publish. Such an instruction
must refuse explicitly. Admission establishes a bounded authority surface; it
does not promise that every possible effect succeeds.

## Corpus inventory and outstanding obligations

Read-only inspection of the pinned runner selects 38 root fixtures after
excluding pages compiled through another fixture's `.sources` list. Additional
mixed-case suffix, debug-metadata and include-resolution checks bring the
interpreter invocations to 45. Three Matrix/Ny twin pairs compare emitted type
and class records. Eight fixtures select the standard library. The existing
`telemetry-relay` exit-only marker is the sole such exception.

The runner also checks 13 named diagnostic rules, 26 foreign-keyword corrections,
declaration contexts and cross-language source/binary import diagnostics.
The handoff's older 32-fixture figure is not the current acceptance set.

Full coverage requires operations absent from profile 1: branches and calls,
recursion, records and choices, classes and dynamic dispatch, faults and
ownership, pointer/buffer operations, file mutation and metadata, time, entropy
and bounded parallel work. The qualification ledger must enumerate unsupported
or unrun obligations explicitly. A reference-runtime pass establishes reference
health; it does not qualify the candidate. Safe refusal of a required positive
fixture leaves that fixture incomplete.

## Evidence covered

- Exact instruction and output comparisons against the pinned reference,
  covering every admitted opcode and console spelling.
- Integer narrowing/wrapping, division and modulo errors, shift counts,
  signed right shift, exact comparisons, casts, locals and decoded strings.
- Denied admission, fixed storage and step/output limits, zero-publication
  rejection of the current instruction and retained earlier output on failure.
- A complete corpus ledger, retaining fixture/runner/tool identities and
  distinguishing candidate results from reference results.
- Focused tests, required regressions, frozen source identity and retained
  unsuccessful attempts, with the final validation receipt recorded below.

## Bounded execution evidence

The host proof compares 126 complete, immutable editions with the separately
linked reference VM: 100 exact successful results/output streams, 20 exact
arithmetic refusals and six deliberately stricter output-policy refusals.
Those six cases cover CR, VT, FF, an oversized write, the total output limit
and a later refusal after previously acknowledged output. They are never
counted as reference-semantic matches. The three compiler fixtures are
recompiled with their pinned options and checked byte for byte; they return
42, 0 and 79, and the console edition writes exactly 26 bytes.

The unchanged upstream runner passes its full obligations through 97 compiler
calls and 45 interpreter calls. All 38 root fixtures are present. The ledger
retains every compiler success/refusal and every interpreter artifact, output
and process result, plus source/stdlib identities and six temporary include
files. The candidate attempts all 45 editions and admits none: 24 unsupported
syntax/forms, nine edition failures, seven syntax failures and five missing
intent failures. These are unmet compatibility obligations, not successful
execution. Only four retained editions even contain an authored intent marker;
there is no injected context or removed registration helper.

An independent verifier rereads raw process records, byte outputs and editions,
recomputes the counts, checks exact arithmetic trap text and verifies all pinned
tool/input identities. Its output retains `candidate_qualified: false` and
`native_execution: false`. Nineteen Rust tests separately exercise witness-bound
receipts, resource boundaries, service failure, retained output and exact
storage capacities; six Python harness tests reject misleading evidence.

The candidate binary used in the proof has SHA-256
`334110c35a645db700ea46be78a6414bcac5ff57fd733973387dd9ef1276c6ea`.
A later Cargo test rebuilt the shared example path with different build-mode
metadata. The exact originally measured hashed executable was recovered and
copied to the evidence directory; size and SHA-256 match the recorded tool.
The original proof and historical command lines were preserved. Reproduction
below copies the executable out of Cargo's mutable output path before testing.

## Final qualification receipt

All 24 final qualification jobs passed against 631 unchanged non-Markdown
source files. This includes every command required by `AGENTS.md`, the bounded
execution proof, the prior host manifest proof, native manifest admission under
TCG and AMD KVM, the existing native edition-delivery regression and the Linux
loader-entry regression. Final jobs had no failing attempt or memory-guard
termination; development attempts are retained separately.

| Host suite | Passed |
| --- | ---: |
| Kernel, default | 648 |
| Kernel, all features | 731 |
| xtask | 425 |
| Native runtime | 37 |
| Program manifest, all features | 18 |
| Program executor | 19 |
| Python proof harness | 6 |

Shared instruction tokenization changes the native admission loader's emitted
artifact while preserving its profile and package fixtures. The exact same
image passed TCG and AMD KVM at 96/320 MiB. Each boot still has four admissions,
six semantic refusals, twelve reads, one fixed loader marker and zero edition
executions, with complete cleanup. Three provisioning refusals per backend
also passed. Independent transcript validation checks the ten exact packages
and canonical reports; its mutation self-test rejects 8,614 changed transcripts
for each backend.

The loader now has 18,666 RX bytes, still within five pages. Its conservative
reviewed stack sum is 13,440 bytes (16 bytes above the earlier artifact), below
the 16 KiB review budget and 32 KiB configured stack. The arena remains 128 KiB.
The review finds no indirect calls, recursive call graph, dynamic stack-pointer
adjustment or extended-state instruction. Checked read-only jump tables remain
within their owning functions. Executor symbols are absent from this vessel.
This is an artifact-specific bound, not a bound for future arbitrary programs.

The existing native edition-delivery image remains byte-identical to its prior
checkpoint. Linux still issues twelve requests, returns from eleven, and stops
at the unanswered sequence-12 `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
No new Linux application execution or exit is claimed. No new VirtualBox proof
is claimed: the external-edition path uses QEMU `fw_cfg`, and the current
VirtualBox runner has no corresponding package provisioning adapter.

| Retained identity | SHA-256 |
| --- | --- |
| Frozen source manifest | `c3aea8c592b520d4def13f1d756ad93a8a9c3a7f61719a482fc65252b2812f96` |
| Final validation summary | `3f60c3dd509e8777e8903cac732c2ac84b741493eba405cab9809bd3562062b7` |
| Full corpus/stdlib input manifest | `4328eb0a154f6f5443fe85065cb6b132c71a6b1f1c2e13993ae10ffe738006a1` |
| Native admission boot image | `20df2f2f40235d08e7fb7fbf2b19f5dd78c4406af5a94bec18066c2a0a0b2464` |
| Native admission client RX | `6508bde6dd4e9cb87afda6764d29c0937fa37c8e7eab8e8b1633ed67d0a14adf` |
| Native admission client ELF | `53eb0142e5a9bba57bbb2cecf90520df798cdba0a24e69c0436190c0bfc3e561` |
| Native admission kernel ELF | `932eedf69db80306e76048741ab0c1544ec9d2178dce27b5b40746517f6c75e8` |
| Preserved native delivery image | `00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b` |

Evidence is retained in the dated `program-execution-2026-11-11` directory
beside the isolated checkout. It contains per-process stdout/stderr, exact
editions, full corpus call records, independent verification, emitted-code
review, both VM backends and the exact recovered candidate binary. Two initial
development compile attempts are retained: an example binding collided with
a tuple-struct name, and tests started before their module file was available.
Both were resolved before the frozen final run. The preliminary proof predates
the final harness tightening and is not substituted for final evidence.

The original Anomalous tracked binary diff and seven unrelated files still
match the preservation baseline. The qualified NIL checkout remains clean at
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. This checkpoint changes neither the
installed SDK nor a stable public ABI.

## Native follow-up

The executor has no task-bound native mediation proof. Native integration must
wait for the corpus gate and then qualify emitted code, memory/stack limits,
service publication, exit results, faults and teardown. The 768-byte manifest
report, one fixed loader marker and zero-status terminal convention retain
their existing meanings. Hosted FFI, dynamic loading and hot reload do not
become native behavior through this candidate.

## Reproduce the bounded proof

Build the host candidate and independent semantic oracle using the separately
qualified NIL checkout at the pinned revision. The oracle links its existing
`nilcompiler_core` and `nil` archives; the proof checks their identities alongside
the compiler and interpreter executables. Its build command is also documented
in `scripts/program_execution_reference.cpp`.

```sh
nil_reference_root=/path/to/qualified/nil-context-registration
cargo build -p anomalous-program-executor --example program_execution_probe
mkdir -p build
cp target/debug/examples/program_execution_probe build/program_execution_candidate
c++ -std=c++17 -O2 -I"$nil_reference_root/nilcompiler/src" \
    scripts/program_execution_reference.cpp \
    "$nil_reference_root/build/context-registration/lib/libnilcompiler_core.a" \
    "$nil_reference_root/build/context-registration/lib/libnil.a" \
    -ldl -pthread -o build/program_execution_reference
python3 scripts/program_execution_proof.py --self-test
python3 scripts/program_execution_proof.py \
    --candidate build/program_execution_candidate \
    --reference build/program_execution_reference \
    --compiler "$nil_reference_root/build/context-registration/bin/nilcompiler" \
    --root "$nil_reference_root" \
    --output build/program-execution-proof-001
```

The output directory must be new. Retain previous attempts and choose another
name for a rerun. `make check` includes the executor's host tests, the proof
harness self-tests, host lint and `x86_64-unknown-none` lint. The differential
proof needs the separately qualified reference toolchain, so it remains an
explicit command rather than an implicit dependency of ordinary builds.
