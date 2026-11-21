---
title: "Canonical Compiler Local Identities"
date: 2026-11-21
tags: [nil, execution, locals, conformance]
status: verified
domain: foundations
---

# Canonical Compiler Local Identities

[ADR 0117](../../docs/adr/0117-admit-canonical-compiler-local-identities.md)
selects host profile 13. Admission keeps the full
`$ny.local.<counter>.<name>` spelling as an opaque function-local identity.
The existing frame slots, kind checks, and call resolution carry those
identities through execution. The pinned compiler emits the generated
spelling for repeated source bindings and keeps the source name separate
from the storage name in debug metadata.

The complete retained invocation 35, `records-and-choices.matrix`, is 7,966
bytes with SHA-256
`19fb31d7b8a1a3eedafc48337d4f5a616aa8cfc9009978e982c7e459bafabe79`.
The pinned VM and profile 13 return signed result zero and the same 34
console bytes. The candidate dispatches 123 instructions and makes seven
console calls. Output SHA-256 is
`646a467a83b233e70b5fba9d4f56db8827c247d5abe8f2e691ed5e4658a888f5`.
The new spelling appears in a helper branch outside this input's execution
path. The complete edition proves whole-body admission, and separate fixtures
execute reads, writes, and indirect calls through generated slots.

The 45 retained raw editions are replayed byte for byte. Profile 13 matches
19 invocations and 13 of 38 root programs. The other 26 raw editions produce
17 unsupported and nine edition refusals. All 45 production attempts refuse,
with 23 intent, 13 unsupported, and nine edition refusals. Invocations 8 and 35
are the only changed outcomes in either lane. Invocation 8 advances to
unsupported mutable indexed storage in raw validation. Completing that
artifact also requires bounded allocation, release, and array extension.

The reference adapter adds a local-name guard to the pinned profile 12
oracle at build time. It verifies the old source hash and records the exact
insertion and generated source hash. The pinned VM still parses and executes
the original edition bytes. The complete replay checks full signed results
and metadata for all matched invocations.

Evidence is retained under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-hygienic-locals-2026-11-21`.

## Focused verification

The final profile 13 proof passes 53 cases. These include 23 semantic
matches, one matching division fault, and 29 validation refusals.
The proof compares 24 cases against the pinned VM and checks 23 pairs
of prior profile lanes. Seven proof self-tests cover exact capture
identity, metadata, local grammar boundaries, and result accounting.
The reference guard suite passes 57 checks.

The profile 13 proof summary has SHA-256
`193cc6b6a5eb91626914ad81f15ac8f0f9cf3c626c7e3c9581204dc976bcb40a`.
The complete invocation 35 receipt has SHA-256
`ae40b375ca1884dd532db6e0598afcb82bfabdfc26153748d836eae13f76f16b`.
The independent reference source check confirms a single 1,039-byte
insertion into the pinned oracle. The generated compilation input has
SHA-256 `fa9cc3cdc47eae81ead41b2475974d0bd2df3c78a5980728de5ba04c53976aa4`.

The independent focused audit has SHA-256
`fbeb6009279e032694fad65dc5f0317522fbc9f4e49e20e0c7448577b474ff9e`.
It checks the pinned capture, exact storage and metadata observations, and
all 749 source hashes. Thirteen altered receipts are rejected by the audit.

Fresh TCG native admission at 96 and 320 MiB retains four admissions, six
refusals, one loader marker, and full cleanup. Both boots record zero native
NIL executions. All five native artifact hashes agree with profile 12.
The independent boot verifier checks 8,614 rejected mutations and records
SHA-256 `58bb636f6025998e565c981c30e85e84dedf3b51be883bd1daf717ea3c598d75`.

## Full qualification

All 48 qualification jobs pass on their first attempt, in 2,372.2 recorded
job seconds. `make check` accounts for 1,356.2 seconds. The run replays
all thirteen earlier language suites with verified reference binaries and
fresh candidate builds. Fourteen suites cover 1,430 boundary cases, and ten
guard jobs cover 535 reference checks. The flow tests pass 171 default and
200 conformance cases. The executor tests pass 109 default and 118
conformance cases.

The 749-file source manifest has SHA-256
`5093cbdc4487ba759d4df9135a58ede08d79c4c7a3915bd3a361c72dd0d7ea0f`.
The independent qualification summary has SHA-256
`62a64b09216f2ee8d47e9d222da0e056a95fbbf578493ee24480fa0ef186b3ab`.
The original checkout's tracked diff and untracked work match the starting
snapshot.

Native program delivery passes with the same boot image hash. Linux-loader
regression uses the retained glibc inputs and reaches sequence 12 after
eleven syscall returns at both tested memory sizes. The observed next
request is `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`. The retained
trace ends at that request with containment and cleanup verified.
