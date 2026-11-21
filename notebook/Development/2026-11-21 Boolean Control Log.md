---
title: "Boolean Control and Float Console Output"
date: 2026-11-21
tags: [nil, execution, control-flow, console, conformance]
status: verified
domain: foundations
---

# Boolean Control and Float Console Output

[ADR 0119](../../docs/adr/0119-add-boolean-control-and-float-console.md)
selects host profile 15. Typed boolean values can drive conditional jumps
and `NOT`. A mixed integer and boolean local join can feed an adjacent
checked `CAST bool`. Each store establishes an exact kind. The join
construction preserves definite initialization and exact callable signatures.

The built-in console call accepts finite binary64 values. It formats them
in classic general notation with six significant digits and sends the
complete payload through the existing console service request. The
executor tests cover negative zero, notation transitions, nearest-even
rounding, authority checks, output budget, service refusal, and cleanup.
The intrinsic console's integer and text contract keeps its previous
kind boundary.

The target is complete retained invocation 9, `bindings.matrix`, with
6,445 bytes and SHA-256
`e4fb6e2fef87f8332980697ca676d819fdc1c21aa9f2c87397cadba1c60f41b3`.
The raw host executor returns signed result zero in 179 instructions and
makes seven console requests. All 24 bytes match the pinned VM. The output
has SHA-256 `bf3320e9ab9a4bfd739251cd519601d28e573fadcc7e88f94f0ee01fa3454471`.
The binary64 literal snapshot agrees on the exact encoding of `0.5`.

The formatter matches a C++ classic stream over 174,957 finite binary64
values. The deterministic sample includes both signs, subnormal values,
the finite limits, and adjacent values around rounding boundaries across
the binary64 exponent range. Its 2,203,490 output bytes have SHA-256
`d71b807e7e24e11bf0caa8347bff8aaf8c7affc3b5fcfce30c25c21c462531ec`.

Evidence is retained under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-boolean-control-2026-11-21`.

## Focused proof and corpus replay

The focused proof covers 100 cases. It records 59 semantic matches, four
matching runtime faults, one finite instruction-limit contrast, and 36
validation refusals. It compares 64 cases with the actual VM and checks 61
earlier profile pairs. The runtime fault cases include three out-of-range
values passed to a joined local's explicit `CAST bool`. The separate
reference guard suite passes 106 cases.

The replay covers all 45 retained invocations and reuses the original 97
compiler receipts and 45 interpreter receipts. All 21 successful raw
invocations match the pinned VM on the complete signed result, console
bytes, and metadata. Coverage is 15 of 38 root programs. The sole change
from profile 14 is invocation 9. The 24 other raw invocations refuse with
15 unsupported and nine edition errors. The 45 production receipts match
the prior profile, with 23 intent, 13 unsupported, and nine edition
refusals.

The proof summary has SHA-256
`9e916bf5de1a735ff4edd1ece8fe8955e6c8ffa0601725ef8fc167ac06321b56`.
The complete target receipt has SHA-256
`c7148a07458361b3ec1f4d6ecbb04cd81881a52609eba0a71afcfc6fd4bd4784`.
The reference guard receipt has SHA-256
`f5f770b2f395a4820b2c0369730912f682f247fa7131f2f97344b9d013218cdd`.
A separate verification path restores all seven edits to the exact
profile 14 reference source and checks the 106 inputs and VM receipts.
It records SHA-256
`b8d3f7753b04c891245c963c4297c247d1930aa653bc7e8be7d34f242de87828`.

The remaining corpus inventory identifies complete invocation 18,
`cross-book-extension.matrix`, as the next bounded target. It requires
text record fields, validation of its empty concrete class marker, and
numeric console output for typed boolean results. The review of all 24
remaining invocations has SHA-256
`051a6c79167356b632d8c2e69a68659aadebd01ccf5b7c03d908bcbf01d5e277`.

## Repository checks and native continuity

The 16 language suites cover 1,632 boundary cases. Twelve reference guard
jobs check 742 cases. Flow tests pass 195 default and 226 conformance
cases. Executor tests pass 117 default and 126 conformance cases.
`make check` passes with 648 default and 731 all-feature kernel tests,
425 tooling tests, and the freestanding checks. The 764-file source
manifest has SHA-256
`36d62e2d9ab8e9d712d5b0fe14e60796719817096c9b693ced4a7f4d1afb6379`.

The native admission path completes at 96 and 320 MiB in QEMU TCG.
Each boot reports four admissions, six refusals, one loader marker,
zero NIL executions, and full cleanup. All five artifact hashes match
the previous checkpoint. The separate verifier checks 8,614 rejected
mutations and records SHA-256
`081303999f24507f93912ebeb657279e38d72a4827b1349b4a76ee20c56c1d59`.

All 52 qualification jobs pass on their first attempt. Summed job time
is 1,562.7 seconds, including 606.7 seconds for `make check`. The
qualification summary has SHA-256
`5e612076583a3e3b9e3622af3a84bb6e7855bf5b16366c1f26b0132a9f078064`.
The original checkout's tracked diff and untracked files match the
starting snapshot.

Native program delivery matches the previous boot image. Both Linux
loader boots use the retained glibc inputs and record eleven syscall
returns. Execution stops at request 12,
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`, with containment
and cleanup verified.
