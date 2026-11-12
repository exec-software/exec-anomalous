---
title: "Native Manifest Loader — compiler bridge and ring-3 admission"
date: 2026-11-11
status: qualified
tags: [native-abi, program, manifest, compiler, ring3, ownership, qualification]
---

# Native manifest loader

This increment connects complete compiler output to the shared semantic-manifest
admission implementation inside a native ring-3 loader. It does not interpret
or execute NIL instructions. Linux remains a separate compatibility route.

## Compiler bridge

The isolated NIL compiler branch `codex/context-metadata-only`, commit
`4dad0512dfd3fa4e351f2816196cb48956f5442e`, adds the explicit
`--no-context-registration` executable-compilation option. Authored annotations
remain unchanged; the compiler omits its generated external registration helper
and corresponding test initialization. Default compilation retains registration.
Explicit foreign calls are not removed. Library generation and read-only modes
reject the option where its per-compilation promise would be misleading.

The LLVM-OFF build passed eight focused tests covering Ny and Matrix, textual
and binary NIL, annotation inspection, default generation, suppression, hosted
execution, test initialization and invalid option combinations. This does not
qualify LLVM native/JIT routes or publish an SDK release. The installed Anomalous
SDK remains untouched. The initial regression run had three **Not Run** entries
because their executable targets had not yet been built; building those targets
and repeating all eight passed.

Retained complete outputs are in
`crates/anomalous-program-manifest/fixtures/native/`, with source, command,
compiler-binary and edition identities in `provenance.json`. Arithmetic is
636 bytes, console 379 bytes, and the larger arithmetic edition 8,261 bytes.
The old compiler outputs remain complete unsupported fixtures. No output was
stripped, normalized or rewritten to make it admissible.

## Native boundary and evidence

The default-off `native_manifest_test` uses a separately named loader geometry:
five RX pages, two private data pages, a 128 KiB RW+NX arena, and a 32 KiB guarded
stack. Its two allowed arena bases are `0x804000` and `0x828000`. Existing native
application and edition-delivery geometries remain separately constrained.
The 18,874-byte emitted loader fits five pages. A conservative review of its
reachable emitted stack paths totals 13,424 bytes, rounded to a 16 KiB review
budget; this is evidence for this artifact, not arbitrary programs.

The common one-task controller retains the existing ownership sequence: import
the sealed source before the execution alias, finish allocator preparation
before guest entry, validate restored continuation/CPU/root/TSS before fresh
borrows, retire execution and aliases before releasing the source. Reports
occupy 9,920 bytes of BSS across ten lifetimes, avoiding a large kernel stack
array. The emitted kernel review is targeted, not a formal whole-kernel bound.

The kernel supplies opaque immutable bytes, task-bound services and an explicit
workload ceiling. It does not parse NIL or depend on the manifest crate. The
trusted loader reads the complete package through checked bounded reads, then
calls the same `admit` implementation used by host tooling. Its private admitted
witness borrows the checked bytes. A witness permitting console output may
produce one fixed **loader diagnostic**; it does not execute the edition's
console instruction.

Every case receives bootstrap delivery and console grants, independently of its
workload ceiling. Refused packages and admitted pure packages issue no console
calls. A bounded canonical 768-byte report is diagnostic evidence only: neither
its decoder nor the kernel converts it into authority. Reports state
`executed:false` and zero workload effects. The host checks exact report bytes,
independently pinned identities, counters and cleanup.

| Case | Expected native result |
| --- | --- |
| Arithmetic, ceiling none | Admitted; no console effect |
| Arithmetic, ceiling console | Admitted; effective workload surface remains zero |
| Console, ceiling console | Admitted; one fixed loader marker |
| Console, ceiling none | Refused: ceiling |
| Large arithmetic, ceiling none | Admitted after three reads |
| Changed edition with stale digest | Refused: identity |
| Console manifest understating its surface | Refused: mismatch |
| Changed authored intent with refreshed digest | Refused: mismatch |
| Nonzero reserved envelope byte | Refused: encoding |
| Complete old compiler output with registration | Refused: unsupported |

Each malformed package has its own correct transport length/FNV pin, ensuring
it reaches userspace admission. SHA-256 remains content identity; FNV is only
trusted provisioning bookkeeping. Missing, short and corrupt provisioning
inputs separately prove cleanup before launch.

One successful boot must report four admissions, six semantic refusals, twelve
reads delivering 14,998 bytes, one loader marker, zero edition executions, and
complete source, authority and resource cleanup. Two RAM sizes and alternating
arena bases exercise fresh lifetimes. The independently written Python oracle
also rejected 8,614 mutations under each retained backend profile.

## Qualification record

The retained artifact directory is
`/home/nathan/Documents/Codex/2026-09-26/l/work/native-manifest-2026-11-11`.
The final plan has 24 jobs over 624 frozen non-Markdown source files, including
all sixteen required repository targets, the native admission proof and existing
delivery proof under TCG and KVM at both RAM sizes, and the Linux regression.
All 24 final jobs passed on their first attempts against the final frozen
snapshot. The source manifest SHA-256 is
`19e10e2f47e71912878508a435ee047ae5946fea044de930c788a921642b9770`.
The independent final summary SHA-256 is `4c8a1e5b5f45c4b1db19c73da9fb3aef5f06c413a15afcbceed02639c2ac804d`.

| Final host suite | Passed |
| --- | ---: |
| Default kernel | 648 |
| All-feature kernel | 731 |
| xtask | 425 |
| Native runtime | 37 |
| Program manifest | 18 |
| Compiler focused CTests | 8 |

The same native admission image passed TCG/Nehalem and AMD KVM/host at 96 and
320 MiB. Its SHA-256 is `7e5a38f722c2ac66fcbc919ca2e8b1beb230b89533a7c3cb626a11132fd0c706`. Each backend also passed missing, short and
corrupt provisioning refusals. The existing delivery proof passed both sizes
and backends with its original two-page/64-KiB/8-KiB geometry, seven reads,
seventeen read refusals and complete cleanup. These are fresh regressions;
no byte-identical comparison with the older delivery image is claimed.

The final standalone verifier checked all job receipts, all 624 frozen sources,
complete compiler outputs, independently reconstructed packages and exact
native reports, both same-image backend comparisons, ten actual KVM command
arrays, emitted guest/kernel identities and original Anomalous work preservation.
All source and report mutation evidence is retained. The Linux regression passed
at both RAM sizes with twelve requests, eleven returns and the unchanged
unanswered request:
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
Linux main entry and exit remain unproved.

Preliminary records preserve the plain-Cargo measurement link failure (fixed by
using the qualified freestanding build flags), the memory-guard termination of
an all-tooling run and the first boot setup, an initial qualification formatting failure, and an omitted expected feature
in the all-feature-list integration test (424 other tooling tests passed). The guard left unrelated processes alone. Corrected builds,
focused tests and the preliminary native TCG proof passed. The final source
freeze was regenerated after formatting; earlier attempts remain separate. The standalone verifier initially assumed
eight E820 records for both backends; the retained KVM map has ten. Its corrected
platform expectations passed both backends and repeated mutation checks without
changing repository sources.

VirtualBox is installed on this host (observed `7.0.16_Ubuntur162802` with its
device nodes). This proof currently provisions external packages through QEMU
`fw_cfg`; the repository's VirtualBox runner has no equivalent package adapter.
No end-to-end VirtualBox manifest proof is claimed. Earlier CPU-capability
refusals recorded on another VirtualBox version are historical evidence only.

The original Anomalous checkout's tracked diff and seven unrelated files remain
preserved. The compiler work is isolated as well; concurrent changes in its
original checkout were observed and left untouched.

## Remaining boundary

The next native step is to qualify the bounded interpreter against a corpus and
connect execution to admitted witnesses and live grants. This work establishes
no general NIL execution, ELF application compatibility, authenticated publisher
identity, signing policy or production desktop readiness. The Linux loader
regression still requires an explicit implementation for its next mmap request;
this increment does not answer it.
