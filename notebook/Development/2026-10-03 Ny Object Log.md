---
title: "Ny Object and Freestanding Startup"
date: 2026-10-03
status: bounded-qualification-complete
---

# Ny object and freestanding startup

This increment follows the completed application CPU/copy/fault profile and
implements proposed [ADR 0049](../../docs/adr/0049-qualify-a-bounded-freestanding-ny-object.md).
Actual pinned Ny-generated code now runs in Anomalous userspace with inspected
startup dependencies and retained terminal cleanup. All 30 final validation
targets passed on 272 unchanged frozen source files. ADR 0049 remains Proposed;
this qualifies one fixed object/startup composition.

## Initial source experiment

The pinned SDK's compiler and assembler built the first integer/f64 probe under
the bounded runner in about one second. Peak scope memory was 23,089,152 bytes;
no scope or host OOM event occurred. The object contained a 504-byte text section,
16 bytes of floating constants, 340 bytes of strings and 25 bytes of BSS.
It imported the four generated-entry runtime hooks plus `strlen`, `calloc`,
and `memcpy` from an unconditionally emitted unused string helper. This changed
the planned dependency policy: preserve the object and provide distinct fatal
unsupported-call stubs, rather than silently link the hosted runtime.

The final source extends the probe with a floating-result global and 50 million
loop iterations. Its nonzero initialized mailbox field and separate initial BSS
checks distinguish copied data from zero-fill and source-level initialization.
At that preliminary stage, native failure/success scenarios, full artifact
inspection and regression qualification were still outstanding. Their final
evidence is recorded below.

## Inspected source and startup contract

The SDK generation is
`456a9a1113d74f6f1c0bcf310cdcd9dc34ead774a584a22770ad7542c4cd676e`, from
clean source `1ea16a3bbbe84c33a7293cbd828478581d16ab4b`, built with LLVM
18.1.8 for Linux x86-64. Its backend selects the host triple, generic CPU and
an empty feature string; it has no target, sysroot or optimization-selection
CLI. This increment qualifies the inspected output rather than declaring a
general Anomalous compiler target.

The checked [source](../../vessels/anomalous-ny-probe/probe.ny) is compiled with
`nycompiler --no-stdlib --no-bundle`, then `nyassembler --unmanaged` produces
the original relocatable object. Managed roots, allocation, strings and TLS
are outside the source contract. The observed object uses ordinary integer and
SSE2 instructions, including conversion, addition and comparison, with bounded
`R_X86_64_PLT32`, `R_X86_64_PC32` and `R_X86_64_32` relocations. Static host
linking resolves these at the admitted low user addresses; only inspected
unwind metadata is discarded.

The [startup shim](../../vessels/anomalous-ny-probe/entry.S) calls the generated
C `main(0, NULL)` rather than bypassing its ABI check or global initializer.
Its supported hooks validate exact call order and System V stack alignment.
The [linker contract](../../vessels/anomalous-ny-probe/probe.ld) bounds RX to
one page at `0x400000`, initializes a 128-byte header at `0x800000` with a
nonzero magic value, and places zero-filled Ny globals at `0x800100`. Startup
verifies the copied magic and initial BSS bytes before Ny initializes globals.
Interrupts use a separately owned TSS privilege stack, preserving the generated
userspace code's red zone.

Two pairs use the same object. In scenario 0, task 0 receives ABI version zero
and reaches the compiler-generated failure path before context initialization.
In scenario 1, it completes 50,000,000 iterations and the context-end shim
injects a failure. Both peers complete with result 50,000,000 and the source
checks the exact floating result 25,000,037.0. The context-end shim observes the
pinned generated wrapper's EAX-to-EBX result preservation across that call;
artifact inspection must verify this private fixture dependency. It is not a
general Ny runtime ABI.

Native qualification requires actual PIT samples in generated code while the
context is active, with eager CPU capture/restore. This is an interrupted
single-current computation proof; it adds no scheduler-preemption claim.
Failure messages and result headers cross current-task checked copies. Exact
armed UD2 faults transfer to the peer, and the last successful task consumes
the retained bootstrap continuation before both owners are reclaimed.

## Preliminary validation and corrections

The final source compiled and assembled successfully, as did the shim. The
first static link with `--orphan-handling=error` rejected unplaced relocation
sections. The linker script now explicitly collects relocation sections in
the forbidden-section guard and asserts its final size is zero: consumed static
relocations are permitted, residual runtime relocations are not. Relinking
passed without rewriting the Ny object.

The same final NIL passed the pinned interpreter route in 16.976 seconds. It
returned process status 128, the POSIX low byte of the expected 50,000,000
result, with no program output. The runner checks that status explicitly.

The first freestanding Ny-feature Clippy check rejected an unused shared
`copy_to_user` method. A narrow feature guard corrected the diagnostic; both
the `ny_object_test` and `ny_object_test,workspace_dev` freestanding profiles
then passed warning-denied Clippy.

Seven focused Ny transcript-oracle tests and xtask Clippy subsequently passed.
The first complete artifact pipeline compiled and inspected the actual Ny
object, checked the independent interpreter route, linked the private startup,
and verified the embedded kernel payload. Its preliminary TCG proof passed at
96 MiB and 320 MiB, accepting 13 records per run and restoring exactly 24,149
and 81,493 free frames. Both runs observed 143 total PIT ticks. Scenario 0
observed zero generated-body ticks for the ABI-rejected task and 47 for its
successful peer. Scenario 1 observed 46/47 body ticks at 96 MiB and 47/47 at
320 MiB for the context-end-faulting task and successful peer respectively.
These observations exercise real compiled code rather than an assembly wait.

The preliminary actual-host KVM proof also passed both memory sizes with 13
records and the same restored free-frame counts. Each run observed 65 total
ticks: scenario 0 recorded 0/12 generated-body ticks and scenario 1 recorded
12/12. Its scope peaked at 1,402,441,728 bytes, with at least 22,886,461,440 bytes
of host memory available and no memory-limit or OOM event.

Artifact mutation tests and overflow hardening are now implemented. The focused
host run passed all 119 xtask tests: 106 preceding tests, six artifact test groups
and seven Ny transcript-oracle groups, followed by warning-denied Clippy.
These checks preceded a fresh 30-target matrix against a freeze of 272
non-documentation files. The earlier native results remain development
evidence; none is counted as final frozen-source qualification.

The default `make check` retains pure artifact/transcript tests and the Ny
kernel feature tests/Clippy profiles, without requiring an installed Ny SDK.
The initial integration put an SDK-dependent image build in that default path;
it was moved to explicit opt-in `make ny-object-inspect`, `make ny-object-image`
and `make ny-object-smoke` targets. Those commands require and verify the pinned
SDK and fail explicitly if it is missing or changed. Nothing is silently
skipped. Both TCG and KVM Ny targets in the final matrix exercise the actual
object/image pipeline.

## Evidence and resource controls

Evidence is retained outside the repository at
an external validation directory (not distributed). The initial probe,
source compilation, failed link, successful relink, interpreter route and
Clippy attempts have separate logs and result/memory summaries. In particular,
`source-results.json` retains the original linker failure and
`source-route-results.json` records the corrected link and independent route.

Builds use two jobs, a 4 GiB soft/6 GiB hard scope limit and a host/scope OOM
watcher. The interpreter/relink scope peaked at 13,852,672 bytes and reported
no memory-limit or OOM events. The first host-development/native proof scope
peaked at 1,523,650,560 bytes, with at least 22,812,741,632 bytes of host memory
available and no OOM event. Final measurements follow below.
No VirtualBox run was attempted in this slice. Threshold is unchanged; the
earlier host-KVM conflict is historical evidence, not a claim about current
VirtualBox availability or an active workspace VM.

## Final frozen-source qualification

All 30 targets passed in 1,293.38 seconds of summed target time. The fresh
`make check` took 270.07 seconds and passed 440 default kernel tests, 449
Ny-feature kernel tests, 483 all-feature kernel tests and 119 xtask tests
(106 preceding tests, six artifact groups and seven Ny transcript-oracle groups).
All 272 frozen non-documentation files remained unchanged after the complete
matrix and normal/workspace image restoration. Preliminary corrections occurred
before this freeze; no partial development matrix is counted toward the result.

Four native boots each accepted 13 records. Every boot completed two scenarios,
four tasks, four dispatches, two contained faults and two normal exits, with
zero scheduler preemptions. Each successful computation returned 50,000,000.
Initialized data, BSS, stack alignment, checked message/result copies and
restored CPU/ownership state all passed their explicit assertions.

| Backend / RAM | Scenario 0 ticks; body ticks task 0 / task 1 | Scenario 1 ticks; body ticks task 0 / task 1 | Total ticks | Restored free frames |
| --- | --- | --- | --- | --- |
| TCG / 96 MiB | 48; 0 / 47 | 96; 48 / 47 | 144 | 24,149 |
| TCG / 320 MiB | 48; 0 / 47 | 95; 47 / 47 | 143 | 81,493 |
| KVM / 96 MiB | 25; 0 / 12 | 40; 16 / 12 | 65 | 24,149 |
| KVM / 320 MiB | 25; 0 / 12 | 40; 12 / 12 | 65 | 81,493 |

TCG reported `GenuineIntel` with the explicit Nehalem baseline. KVM reported
the actual host's `AuthenticAMD` profile. Timer counts vary with execution and
host scheduling; these observations prove interruption of generated code, not
a performance benchmark. Both backends used a byte-identical Ny proof image.

The final matrix peaked at 4,297,084,928 bytes (about 4.002 GiB) under the
6 GiB hard cap and 4 GiB soft limit. Host available memory stayed at or above
21,109,440,512 bytes (about 19.660 GiB). There were 323 soft-limit reclaim
events, zero hard-limit events, zero scope OOM events/kills and zero host OOM
kills. The watcher did not abort the run.

Normal and workspace image restoration succeeded in 0.5 seconds each; both
hashes match the preceding checkpoint. No VM was left running, and the
interactive scope is inactive. No VirtualBox run was attempted in this slice;
Threshold remains unchanged.

| Artifact | SHA-256 |
| --- | --- |
| Ny proof image, TCG and KVM | `88d507ad2ac086f9e3f331be9a32f52bd250c34eab0b3d2798ec3e2a138dfa4a` |
| Restored normal image | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| Restored workspace image | `65d6cfe39038ac6c078139e9d052a3f790f31eb8b1a018ebd3243cf904251077` |
| Original Ny relocatable object | `6cbdcf729874b4de55176b0a3eb379c61f5dc4cb719b6671ff0196e98e13e58b` |
| Linked user executable | `6bead497ce3e71c213c2334259fd3ebdab14eef39e9e02ff6cafe800140c17d2` |
| Embedded RX bytes | `fde7b2e3224c9dba687390c61da998639fac615871f0ef76cf732ce26570cf50` |
| Embedded initialized data | `e6a843b0349daab3cb4d313bd22d2ee78a54151d552bbf581293da690f12a969` |

The external evidence directory above retains `summary.json`,
`matrix-final-results.json`, source-freeze comparisons, serial transcripts and
per-phase resource summaries. `final-artifact/qualification.txt` records the
exact source, shim, linker, SDK tool, NIL, object, executable, extracted-data
and wrapper hashes, together with interpreter status 128 and result 50,000,000.

## Remaining gates

This proves no hosted Ny runtime, general target or arbitrary object loader,
allocation/string library, TLS, application-service ABI, native Antimatter,
HAG transport or privileged Ny migration. With this bounded qualification complete,
task-bound surface/input services and a native Antimatter frontend are next;
workspace persistence follows. Further language/runtime requirements must be
qualified as the actual application needs them.
