---
title: "Bounded Indexed Storage"
date: 2026-11-21
tags: [nil, execution, arrays, storage, conformance]
status: verified
domain: foundations
---

# Bounded Indexed Storage

[ADR 0118](../../docs/adr/0118-add-bounded-indexed-storage.md) selects host
profile 14. Integer array handles preserve shared writes and growth. Explicit
`ALLOC i64` uses an opaque, zero-initialized buffer, and `FREE` clears its
cells and revokes every alias. Indexed access to a released buffer reports
`buffer_released`. The pinned VM instead represents allocation results as
integer addresses, with different behavior for stale addresses.

Arrays and buffers use the existing fixed 128 descriptors and 1,024 backing
cells. A buffer release retains its allocation charge. Array append either
extends the tail or moves the extent and charges its full new length. The
descriptor identity survives movement so earlier aliases still see the same
array. Both operations scrub retired cells.

The target is the complete retained invocation 8, `aggregates.matrix`, with
11,250 bytes and SHA-256
`04973bd3779931a6128e4e4e9250ecd954dca79ea0ad9875a2876cea0d1f190a`.
The candidate's full signed result is zero. It dispatches 231 instructions,
makes 15 console calls, and emits 70 bytes with SHA-256
`5d98b21c984b0f1bb5fbc7e067f0e5396d0126b820d256062baaef670eee32e9`.
The pinned command-line interpreter agrees on output and the process exit
status. The original edition executes all new operations and observes the
array write, changed length, and sum from the explicit buffer.

The development replay preserves all 19 earlier raw matches. It adds
invocation 8, reaching 20 of 45 invocations and 14 of 38 roots. Invocation 9
advances to a syntax refusal in raw validation. The raw refusal totals are
15 unsupported, one syntax, and nine edition refusals. All 45 production
receipts match profile 13, with 23 intent, 13 unsupported, and nine edition
refusals.

Evidence is retained under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-indexed-storage-2026-11-21`.

## Focused proof

The pinned VM returns signed result zero and the same 70 bytes for the
complete target. All 20 successful raw editions agree on full results,
console bytes, and descriptive metadata. The proof replays the original
capture and retains the same edition bytes and source paths.

The 102 focused cases include 45 semantic matches, one matching division
fault, 11 finite runtime policy contrasts, one candidate-only repeat-release
case, and 44 validation refusals. The suite compares 57 cases with the VM
and checks 55 earlier profile pairs. Seven proof self-tests pass.
The reference observer records actual allocation identities, lengths, and
release state. The VM owns the element values and executes every operation.
All 101 reference guard checks pass.

The proof summary has SHA-256
`bf424f068525372194852e9f4d601c388cd26d43f9d238684d0f02a9c72a054b`.
The complete target receipt has SHA-256
`df95eec0bced1a4bce1e17b3ac3cf2a109f435ba257e456f50e38c0ece545ece`.
The guard receipt has SHA-256
`73f27a4b23484a922493d6e2cd41c32a61b13fc9ac93accfdab29ea30389f629`.
The 756-file source manifest has SHA-256
`3dd1abbc7c2633ba483f1afeccc563ecdfaf1e7497566007a603aafecf1db8b2`.
The independent proof audit checks all 756 source hashes and 481 process
receipts. It has SHA-256
`8ac412e6ec5a02a9f2090cbc1715051ea9c97267c560149146db2a270cb3b8d4`.
The independent reference audit reverses the exact adapter changes and
checks all 101 reference guard receipts. It has SHA-256
`4e805d597faff8c92e2a458818fc0c68d6286f416510404fa2c3baaf7ce1bc98`.

The first issue reached in the remaining `bindings.matrix` edition is a
boolean branch. A complete run also requires boolean negation, compatible
short-circuit joins for the emitted boolean and integer temporary, and
finite floating-point console formatting.

## Repository checks and native continuity

The program flow tests pass 184 default and 214 conformance cases. The
executor tests pass 114 default and 123 conformance cases. The complete
`make check` passes, including formatting, lint, and freestanding builds.

The fresh native admission runs at 96 and 320 MiB each record four
admissions, six refusals, one loader marker, and full cleanup. The five
native artifact hashes match the previous checkpoint. Both boots record
zero NIL executions. The independent boot verifier checks 8,614 rejected
mutations and records SHA-256
`26db5f5b7eaad653e24cd5548fec92014440c5fca58ae79ee20a99d16b838086`.

All 50 qualification jobs pass on their first attempt. Summed job time is
1,512.1 seconds, including 622.1 seconds for `make check`. The run replays
all 14 earlier language suites against fresh candidates with verified
reference binaries. The 15 suites cover 1,532 boundary cases, and the
11 reference guard jobs cover 636 checks. The qualification summary has
SHA-256 `013629560120ff3eeecb8b9c9017b857ff1c31cff4814bd8f3df24117477a710`.
The original checkout's tracked diff and untracked work match the starting
snapshot.

Native program delivery matches the previous boot image. The Linux-loader
regression uses the retained glibc inputs. Both memory sizes record eleven
syscall returns and stop at the observed sequence-12
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)` request, with
containment and cleanup verified.
