# Bounded module globals and integer text — 2026-11-13

This checkpoint adds explicit host profile 8 under
[ADR 0112](../../docs/adr/0112-add-bounded-module-globals-and-integer-text.md)
for module-scoped `i64` state and
integer-to-text conversion. Its target is the complete unchanged
`two-books.matrix` edition. It changes no kernel, native service, desktop or
Linux-loader behavior.

## Implementation and reference boundary

The whole edition interns up to 128 global names from exact typed
`LOAD_GLOBAL <name> i64` and `STORE_GLOBAL <name> i64` instructions. Simple names
and qualified ASCII identifier segments joined by `::` retain their exact case
and spelling, with a 128-byte total bound. Dead instructions and unused helpers
consume slots too. A load-only name is valid: registration establishes an
addressable slot, not an initialized value.

Each execution owns a fixed array of optional integers. All frames share those
slots. A store consumes one integer and overwrites its slot; a load returns the
complete stored value or reports `global_uninitialized`. Missing values never
become zero. Initializer functions such as `__globals_init__` are ordinary
functions and run only when the program calls them. Globals and initialization
state are cleared at entry and on every successful or refused exit, alongside
all existing arenas, operands and frame locals.

Global names occupy a separate namespace. Explicit global operations do not
change lexical-local resolution, indirect-call provenance or builtin effect
classification. Global values cannot be text, arrays, records or callables.
The CFG proves scalar operand kinds but leaves cross-function initialization
order to execution, preserving the pinned VM's missing-global failure.

`CAST text` accepts only a proven integer. It produces exact signed decimal
ASCII, without grouping or a leading plus, using at most 20 bytes. Formatting
handles `i64::MIN` through its unsigned magnitude and uses no float or heap
conversion. The existing 65,536-byte decoded text arena owns the result;
concatenation and formatting share its cumulative budget. Complete capacity
checks precede writes, and aliases, arrays and helper returns retain their
computed text until execution ends. Other casts and implicit coercions retain
their earlier restrictions.

The reference is compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. Its global map resets for each
execution, a missing load throws `Global not found: <name>`, and integer-to-text
casting uses decimal `std::to_string`. The new C++ oracle runs those actual VM
operations while guarding integer global values, qualified names, bounded
storage and callable/record provenance. It does not simulate the global values
or substitute candidate formatting results.

Earlier profiles retain their grammar and envelope tags. Production admission
continues to require authored main intent; the separate raw conformance route
does not manufacture it. Record-scoped context remains separate. No corpus
helper, annotation or unsupported declaration is removed to gain coverage.

## Development evidence and source freeze

The initial focused run passes 131 conformance flow tests and 85 conformance
executor tests, plus three flow and one executor compile-fail witness tests.
Independent runtime reviews found no defect. All 43 new oracle guards pass.
The complete development proof passes all 105 cases and reaches 13 raw corpus
matches, with the expected 20 unsupported, nine edition and 16 intent errors
in the separate production lane. Independent retained-evidence auditing agrees.

An early harness self-test invocation ran while its selftests were still being
written and reported zero tests; that output is retained but receives no
validation credit. The completed harness subsequently passes all seven
selftests. No implementation or expectation correction was needed after the
first complete differential run.

The final run starts at `64a4696d1cf22c0f316de4bd1c64d4f0299fcbb6` on
`codex/nil-module-globals`. Its 711 non-Markdown source files are pinned by
`qualification/source-sha256.json`, SHA-256
`c525551f0ec19be80a91a5ed8d480485cc58e2b55b75c402cd29a355ca90b4ff`.
Implementation, probes, proof scripts and oracle sources remain fixed during
qualification. Documentation and external audits are completed separately.

The final plan preserves all eight earlier proof suites (126, 77, 61, 27, 55,
87, 103 and 115 cases), their 15 immutable proof/oracle sources, and the
unmodified upstream runner. It retains all 97 compiler calls, 38 roots, 45
interpreter invocations and auxiliary obligations, including the existing
telemetry exit-only exception. Refusals remain unmet semantic coverage.

## Independent language evidence

All nine final language proof suites pass. The new 105-case suite records 52
exact semantic matches, seven matching runtime faults, four finite resource
contrasts and 42 validation refusals. It includes 63 full-i64 VM comparisons,
105 production counterparts, 63 prior-profile raw/production pairs and one
complete corpus prior-profile pair. Runtime policies and validation refusals
receive no semantic-match credit.

The complete 4,214-byte `two-books.matrix` edition (invocation 43) has SHA-256
`0906256ac45e18e81df635e70f9657fb6c74cc42e5907add63de4b1fd1d135e5`.
It returns zero after 92 candidate instructions, with five acknowledged console
calls and 30 output bytes: `3\n42\nMODULAR BILLING\n184\n1799\n`.
All twelve earlier raw matches remain. Thirteen of 45 raw invocations match,
including eight of 38 roots; the other 32 refusals are 23 unsupported and nine
edition errors. Production independently refuses all 45: 20 unsupported, nine
edition and 16 intent errors. No bytes or authored metadata are rewritten.

Final global proof summary SHA-256:
`671d587d983fa3ce467a9886a9e41d396ea350cd0b3fbe9b7d099726efa76394`.
Independent retained-evidence summary SHA-256:
`42167233f98a1d18ce650dda62ba13417ab27c60e53a1a9075e9d22fdc7f0742`.
Raw candidate SHA-256:
`b205dd92da43e9385f17a2293b9e0d63aebc893132111d68d5198a02c0650dd0`.
Production candidate SHA-256:
`56cd76576ae69ba2b7d0d60f47328810fd16676837a65bd7759feb383acba1d3`.
Independent C++ oracle SHA-256:
`fe77cef99cd0590e0be96af2e15546e9747edffec9df62425a7602121dca19d9`.

The five oracle-guard jobs pass one text, ten array, 19 callable, 42 record and
43 global checks. The new guard cases exercise missing and shared global state,
name/type bounds, namespace collisions, blocked local fallback, integer-only
formatting and shared text-allocation limits. These are 115 guard fixtures,
not additional semantic corpus matches.

## System qualification

All **44 final jobs pass on their first attempt**, totaling 1,400.1 seconds of
serialized work; `make check` accounts for 580.1 seconds. The run includes all
required AGENTS smoke targets, all nine language proofs, five oracle-guard jobs,
native admission/delivery, manifest qualification and Linux-loader entry. No
memory or timeout guard fired.

The independent aggregate verifies exact commands and results, all 756 boundary
cases, 115 oracle guards, complete source/tool/input identities and preservation
of the original checkout. Final host counts are flow 109/131 and executor 76/85
(default/conformance), kernel 648/731 (default/all features), xtask 425, runtime
37 and manifest 18. Three flow and one executor compile-fail witness tests pass.
The nine Python harness suites pass 6/5/4/5/5/6/7/7/7 selftests. Both freestanding
host-conformance feature exclusions pass.

Retained artifacts live outside Git under
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-global-2026-11-13/`.
The final aggregate `qualification/summary.json` has SHA-256
`49a049f0874bc1c592c949458ff7a0d67ba6fa12e2f6101b8bde7a60427de744`.
All 711 frozen source files remain unchanged. The original tracked diff and
seven unrelated files are preserved.

## Native continuity and remaining gates

Fresh TCG native-admission runs at 96 and 320 MiB retain four admissions, six
refusals, twelve reads, one fixed loader marker and zero NIL execution. The
independent verifier passes all 8,614 mutation checks. Boot image, kernel,
client, RX image and payload bytes match the reviewed checkpoint exactly.
Historical AMD KVM evidence is reused only under those exact artifact/source
identities; this slice claims no fresh KVM or VirtualBox run.

Native program delivery retains its reviewed boot-image identity. Fresh
Linux-loader runs at both memory sizes still record twelve requests and eleven
returns, ending at unanswered sequence 12:
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
There is no Linux program exit or new compatibility claim.

Full corpus qualification, native NIL execution, new task/service authority and
general Linux compatibility remain separate closed gates. Per-execution module
state does not become shared host process state or persistent storage. Production
admission still requires authored main intent; no corpus metadata is invented.

The read-only next-slice audit recommends bounded nominal enums with unit or
`i64` payloads. The complete `names-and-case.matrix` edition still contains an
unused enum declaration, while `value-position-then.matrix` exercises variant
operations. Together they could raise coverage to 15 of 45 invocations and ten
roots only after a new proof. Source-debug metadata and broader scalar types
remain separate possible slices.

Sixteen direct probes against the pinned interpreter resolve the extraction
boundary: full signed-i64 values survive construction, extraction, helper
returns and aliases. The permissive behavior concerns payload count, not integer
width: extraction can ignore trailing fields or push nothing for a unit value.
Tag tests also accept strings and do not enforce nominal metadata. A future
profile therefore needs declared nominal identity, exact payload-shape checks,
opaque construction and explicit policy comparisons for refused legacy cases.
No i32 payload cap is justified by these probes.

The final `next-slice-audit.json` has SHA-256
`a35cbfa4a90325f3328829c61c534acf174744a3f64607798fc48bcc5cd94c05`.
`enum-primary-semantics.json` has SHA-256
`4637772fedd955c7b19ee044e979ff36caefab2d70fd5fc50f40e563eca08f76`.
The separate exact-target process review is retained as `target-review-final.json`,
SHA-256 `864dd93e7b8b8624344dab4138d1e7a1a664e761ba6c9950b960d706a0a6ccb3`.
These audits change no source or qualification claim.
