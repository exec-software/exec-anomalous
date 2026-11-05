---
title: "Eidolon and Ny Integration Log"
date: 2026-09-09
tags: [eidolon, ny, codec, sdk, framebuffer, experiment]
status: active
domain: tooling
---

# Eidolon and Ny integration log

## Direction

The serious Ny proving project belongs in Anomalous rather than beside it.
Eidolon is a lossless framebuffer-trace codec: an immediately useful host tool
for QEMU visual evidence and a future candidate for governed display, recovery,
and Oracle artifacts. Proposed ADRs 0016 and 0017 hold the normative language,
SDK, format, isolation, and privacy decisions while they remain under review.

The key architectural fit is not only performance. Both projects treat runtime
diagnostics and AI-facing context as designed data rather than incidental text.
Ny annotations could eventually carry codec invariants, schema identities, and
test-vector links into a Reliquary, but code and embedded context need
independent trust, provenance, disclosure, retention, and stripping policy.
That should be proved with an inspection tool before the metadata grows.

## Initial implementation

The first checked implementation is a standard-library-only Python oracle under
`eidolon/reference`. Experimental draft 1 has fixed little-endian file and frame
headers, RGB8 geometry, rational timing, 16-by-16 default tiles, raw, solid,
pixel-RLE, unchanged, and XOR-RLE modes, and CRC32C for headers, payloads, and
decoded frames. It is deterministic for identical inputs and options.

The decoder treats all wire values as hostile. It checks arithmetic and local
budgets before allocation, validates exact frame and tile structure, forbids
unknown modes and flags, rejects trailing bytes, and returns no frames until the
entire trace is checked. This whole-trace behavior is safe enough for the first
oracle but bounds useful duration through its total-output limit. A future
streaming API should preserve a last-verified reference and isolated candidate
buffer so one corrupt frame cannot become visible.

The first corpus is deliberately small and committed as pixels plus a golden
container digest rather than an opaque generated binary. Tests also build edge
tiles, temporal changes, CRC-valid structural corruption, zero-length runs,
resource-limit failures, and a CLI PPM round trip.

## SDK contract

Eidolon refreshes Ny from the sibling checkout into ignored immutable
generations under `eidolon/.sdk`. `eidolon/sdk` is an atomically changed symlink.
The SDK root contains all canonical `ny*` tools directly, with `nyinterpreter`
as the sole runtime name and no `bin/` indirection. `nylib`, `nystd`, compiled
docs, manifest, and license complete the payload.

Strict standard-library compilation is intentional. If any `nystd` module
cannot produce checked binary NIL, refresh fails and the previous generation
remains active. An unchecked SDK would hide exactly the incomplete language
surface Eidolon is meant to find.

## Observed Ny gates

The initial audit found declarations for byte and handle APIs without complete
runtime behavior, route-inconsistent byte-width and pointer semantics, missing
native argument/file behavior, and partial aggregate and result lowering. At
that checkpoint the honest Ny entry point reported that it was not an encoder.
Inline-assembly syntax is now recognized but compile-rejected fail-closed
because no backend lowering exists; no emitted behavior is claimed. The feature
ledger maps each blocker to a codec test instead of guessing around it.

The first implementation slice should be portable in-memory CRC32C and keyframe
tile coding through `nyinterpreter`. File I/O follows true octets. Native code
follows interpreter cross-conformance. Assembly follows portable Ny plus CPU
feature dispatch and disassembly inspection.

## Interpreted vertical slice observed

On 2026-09-09 the placeholder entry point was replaced by a reusable Ny codec
module and a deterministic conformance entry point. The slice implements
table-free CRC32C plus clipped raw, solid, pixel-RLE, unchanged, and XOR-RLE
tile selection, emission, and strict decode. It uses Ny arrays for caller-owned
frames, pointer-backed candidate and payload buffers, structs for owned results
and tile geometry, enums for wire modes and attributable failures, and bitwise
operations for CRC and temporal deltas.

The fixture is useful because it is small without being trivial: the key frame
chooses raw/raw/raw/solid across four clipped tiles, and the delta frame chooses
XOR-RLE/unchanged/unchanged/solid. A separate four-pixel vector forces
pixel-RLE. The Python harness independently extracts the oracle payloads and
compares every Ny-emitted byte rather than trusting a Ny self-test message.

Observed command:

```sh
NY_COMPILER="${NY_SDK}/nycompiler" \
NY_INTERPRETER="${NY_SDK}/nyinterpreter" \
python3 -m unittest discover -s tests -p 'test_ny_codec.py' -v
```

All six differential tests passed through both textual and binary NIL. The
portable CRC returned `0xe3069283`; key and delta payload bytes and CRCs matched
the oracle; all decoded pixels matched; and malformed lengths, runs, modes,
trailing data, and temporal misuse produced the expected typed statuses. The
Ny compilation did not use `--no-typecheck`.

During the same toolchain iteration, `nyinterpreter` gained real whole-file
`array<u8>` reads and writes plus interpreter argument access. Eidolon has not
yet claimed that work as a file codec: that public slice intentionally
uses range-checked `i32` logical octets around pointer buffers, and complete
file/frame records are still absent. The next falsifiable step is to emit and
consume one complete draft-1 container through the Ny byte APIs, then compare
the file itself and decoded PPM raster against Python. The compiler used
for this run has LLVM disabled, so native behavior remains unobserved.

## Integration audit and resource hardening

Later on 2026-09-09, a bounded audit tightened the reference tool without
changing draft-1 bytes. PPM and container inputs are now opened as regular
files and read with a hard maximum on the opened descriptor, so a special or
growing input cannot evade a path-size check. Aggregate decoded-frame and
incremental encoded-payload budgets are checked before materialization or
buffer growth. Wire-width inputs now reject non-integer values with the codec's
structured exception rather than leaking a `struct` failure.

**Observed:** `make test` passed 26 tests: 20 Python oracle/CLI tests and the six
Ny differential tests discovered through the sibling non-LLVM build. The added
tests cover pre-materialization aggregate rejection, strict wire integer types,
pixel-RLE zero/underfill/overflow rejection, nonblocking refusal of a FIFO,
private-path redaction on failure, and dangling output-symlink refusal.
`python3 -m py_compile reference/*.py tests/*.py` and
`bash -n tools/refresh_ny_sdk.sh` also passed. An ad hoc, uncommitted
`python3 -` diagnostic (size/option seed `0xe1d010`, so not retained as
conformance evidence) completed 1,000 randomized small-frame round trips and
5,000 malformed-input decodes without an unexpected exception.

## Vendored SDK activation observed

On 2026-09-13, `make -C eidolon sdk` built and activated clean Ny revision
`7ecb2568ff742850284280c8782e36c1df62a82e`. The build detected LLVM 18.1.3
and passed all 124 registered Ny CTests before publication. The immutable
generation is
`18302cceefd3c36994274bc91591678fa6881ca3e15e96b6c968875e6a97c505`;
`eidolon/sdk` points to it under `.sdk/versions`.

The manifest records a clean source tree, 63 standard-library source files,
15 strictly checked logical modules, 77 binary library artifacts, and 17 HTML
documentation pages. The SDK root contains `nycompiler`, `nyinterpreter`,
`nyassembler`, `nylinker`, `nylsp`, `nydebugger`, `nyschema`, and `nyagent`
directly, followed by `nylib`, `nystd`, `docs`, `manifest.json`, and `LICENSE`.
There is no `bin/` indirection or retired runtime alias.

`make -C eidolon check` then passed all 26 codec tests using that activation;
the six Ny differential tests also passed independently through the SDK-local
compiler and interpreter. This closes the refresh and interpreter integration
gates. At that checkpoint, the Eidolon Ny slice still intentionally tested
textual and binary NIL execution rather than claiming native codec parity or optimized
inline assembly; those remain explicit next stages.

## Complete interpreted file path observed 2026-09-13

The next Ny slice now reaches the filesystem without relaxing the codec's
security posture. `format.ny` encodes and validates complete experimental
draft-1 file and frame records. `ppm.ny` parses the strict P6 subset, including
comments, exact raster length, and an inclusive 4096-byte complete-header
budget. `cli.ny` exposes three real commands:

- `encode OUTPUT.eidolon FRAME.ppm [...]` reads every explicitly selected frame
  through a bounded no-follow regular-file operation and atomically replaces
  one container only after the complete encode succeeds;
- `inspect INPUT.eidolon` validates the complete trace and emits stable
  path-free key/value geometry, timing, mode-count, and CRC evidence; and
- `decode-frame INPUT.eidolon INDEX OUTPUT.ppm` validates every declared frame
  before atomically publishing one canonical P6 file with no-replace semantics.

The single-frame decode projection is deliberate. The Python oracle can stage
and rename a complete output directory, but Ny does not yet expose an atomic
directory transaction. Repeating atomic file writes and calling the batch
failure-atomic would be false, so the CLI makes one publication the unit of one
invocation.

Observed development command while the replacement SDK was being prepared:

```sh
NY_COMPILER="${NY_SDK}/nycompiler" \
NY_INTERPRETER="${NY_SDK}/nyinterpreter" \
NY_STDLIB="${NY_SDK}/nystd" \
PYTHONDONTWRITEBYTECODE=1 \
python3 -m unittest discover -s tests -v
```

At that development checkpoint, the hardened suite contained 51 tests: 34
Python oracle/CLI tests, nine Ny file/format integration tests, and eight
tile-route tests. The complete CLI
harness independently compiles textual and binary NIL and proves byte-identical
multi-frame containers, exact PPM output for every selected frame, exact mode
histograms, full-width unsigned decode timing, full-u32 encode timing with a
greater-than-`INT32_MAX` timestamp product, and equal rejection of CRC, mode,
timestamp, zero/max geometry and frame counts, flags, duration, uncompressed
size, maximum payload size, truncated headers, and trailing data. PPM evidence
also fixes magic at byte zero, preserves semantic geometry/maxval categories for
decimal overflow, and aligns the exact 4096-byte comment/separator boundary.
Security cases cover exact descriptor-sized reads, short-read races,
post-read metadata mutation, growth, nonblocking FIFO and no-follow symlink
refusal, path/basename/fragment-redacted failures, preservation of existing and
dangling outputs, path-neutral staging names, injected-failure cleanup, and
refusal to replace an encode destination symlink or FIFO while preserving
ordinary regular-file replacement. Cleanup is best effort: host cleanup
failure as well as uncatchable termination can orphan private staged bytes, so
the output parent requires an explicit retention sweep.

A restrictive-umask regression exposed that `tempfile`'s requested mode is
still filtered by the caller umask: under `umask 0777`, encode previously
published a mode-000 container and decode created an unusable mode-000 staging
directory. The POSIX oracle now applies `fchmod(0600)` to the still-open encode
descriptor, `chmod(0700)` to the decode staging directory immediately after
creation, and `chmod(0600)` to every staged PPM. A subprocess regression runs
both commands under `umask 0777`, verifies exact final modes and content, and
finds no staging orphan. Injected fchmod, directory/frame chmod, write/sync,
and publication failures exercise the ordinary cleanup paths.

The in-memory `src/main.ny` plus `src/codec.ny` corpus now matches the same
parsed oracle through LLVM JIT and AOT as well as textual and binary NIL. This
is concrete native evidence for structs, enums, word arrays, pointers,
allocation, indexing, CRC, and tile modes. It is not full native CLI evidence:
byte-specialized arrays, secure file calls, complete records, and PPM
publication have not crossed that route. A native returned-struct lifetime leak
also remains open, so repeated production use is not yet safe even though the
bounded native corpus completes correctly.

### Interpreter memory profile and corrected local gate

The codec still uses range-checked `i32` logical octets internally, but the VM
stores them as generic values and creates several transient whole-buffer
arrays. Logical bytes therefore do not bound resident memory. GNU
`/usr/bin/time -v` on the 2026-09-13 textual-NIL development build measured:

| Operation | Logical data | Maximum RSS |
| --- | ---: | ---: |
| Encode 320×200, one patterned RGB8 frame | 192,000-byte raster; 193,392-byte container | 119,384 KiB |
| Encode 640×480, one patterned RGB8 frame | 921,600-byte raster; 927,692-byte container | 477,748 KiB |
| Inspect that 640×480 trace | 927,692-byte container | 477,200 KiB |
| Decode that 640×480 trace to PPM | 921,615-byte PPM | 624,612 KiB |

The earlier 16/32/64 MiB logical profile could thus authorize unsafe multi-GiB
RSS. Frame, payload, container, and aggregate decoded limits are now 1 MiB each.
That empirical gate retains one practical 640×480 default-tile framebuffer but
is explicitly not an RSS guarantee or production-size promise. A
byte-specialized array representation, fewer full-buffer copies, and accurate
VM allocation accounting are required before raising it. Exact `array<u8>`
conversion remains confined to the secure I/O boundary.

### Compiler defects exposed by the vertical slice

The first strict CLI compile found two Ny front-end resolution defects. Both
were repaired in Ny during the same integration iteration; the temporary
workarounds and exact diagnostics remain here as regression evidence.

The parser confused a field-bounded comparison with generic arguments. The
minimal source shape was:

```ny
struct token_result
    start: i32
    finish: i32
end struct

fn consume(values: array<i32>, item: token_result)
    let mut offset = item.start
    while offset < item.finish
        let value = values[offset]
        offset = offset + 1
    end while
end fn
```

The strict compiler reported the first loop-body line with:

```text
Expected '>' after generic arguments
Unexpected generic arguments not followed by call or struct init
```

Copying `item.finish` to `end_offset` and using the equivalent bounded
`while offset != end_offset` initially avoided the ambiguity. Ny's parser was
then repaired to use bounded type-grammar lookahead rather than an arbitrary
forward `>` search; the natural comparison above was restored and all 33 tests
in the then-current Eidolon suite remained green.

The second defect appeared when multiple modules declared same-named helpers.
Both a source-qualified `ppm::status_code(image.status)` call and an unqualified
call inside `ppm` emitted `CALL format__status_code` in textual NIL. Similarly,
a local `format.failed_encode` resolved to the private helper in `codec`, which
the type checker then reported as returning `codec::encoded_payload` where
`format::encoded_trace` was required. Unique names (`ppm_status_code`,
`trace_status_code`, and `failed_trace_*`) provided the safe temporary
workaround. Ny's unqualified-alias precedence was then repaired so an imported
name from another module cannot outrank the current module's qualified
declaration. The exact reproducer now prints the expected value through textual
and binary NIL. Eidolon keeps the unique helper names because they communicate
ownership clearly, not because the compiler still requires them. These cases
should remain compiler regressions; allowing project correctness to depend on
declaration order would make machine-authored multi-module code especially
fragile.

## Committed SDK activation and final evidence — 2026-09-13

Ny was committed and pushed before the project SDK was rebuilt. The activated
SDK has this reproducible identity:

- Ny revision: `3090713791df857ccd37e1407ad96e2262dba5c8`
- immutable generation and manifest SHA-256:
  `6837b05c8a99fbd4c12e8323b883299da37151564d96288d26632dd437623467`
- manifest source state: `worktree_dirty=false`
- compiler SHA-256:
  `af227f3e88584e80c21f22cc6ff210273e03fdd7823dd37a615cad548ecefa5a`
- interpreter SHA-256:
  `3a60d1594a34fea1a7ccc82003959227937d9c3e22a215839be2e4f60dab58b9`
- assembler SHA-256:
  `9d328c1e810ca26c0516929951b0ca4ce27ecb9bfb651f6b82b9a2e9f813cc07`
- linker SHA-256:
  `5dbda4f73efc063089da26889d57a2dc363c8d6ea79474a088886ee3377ae69d`
- language-server SHA-256:
  `2adb445271f1c3b6686e3f03942c6c85f386e2b3a8e0dfad3e0e22375fdffa42`
- debugger SHA-256:
  `d079718bd3fc248db9f551059bb0e4c500dcf4fae1c4b946039cb6f2471fdd1f`
- schema-tool SHA-256:
  `3649e5681c537cb4e42362ec9a53b07eab117317679b486ec71883573bdadf0c`
- agent-tool SHA-256:
  `99a81708a595590632eebd6d2d0fc48a8d1052587eaa918141f64d5275652cd8`

The SDK root contains exactly the eight public tools plus `nylib`, `nystd`,
`docs`, `manifest.json`, and `LICENSE`; it has no `bin/` indirection or retired
runtime alias. Packaging strictly compiled 15 logical standard-library modules
and built 17 documentation pages. The SDK builder's clean Release/LLVM run
passed all **173/173** Ny tests before activation.

Final Eidolon commands pinned `NY_COMPILER`, `NY_INTERPRETER`, and `NY_STDLIB`
to that one activated generation. The complete suite passed **54/54**: 34
independent Python reference tests, nine Ny file/format integration tests,
eight Ny tile-route tests, and three fail-closed tool/stdlib resolver tests.
The focused Ny selection passed **20/20**. The isolated LLVM JIT and AOT codec
oracles passed **2/2 with zero skips**, and `make ny-cli` produced both checked
textual and binary NIL for the four-module file CLI.

This activation establishes reproducible interpreted file/PPM behavior and
native in-memory codec behavior. It still does not establish a native file CLI,
streaming I/O, multi-output transaction, SIMD/inline-assembly path, or native
aggregate lifetime management; those remain the next production gates.

## Conformance transcript idea

The machine-readable numeric transcript used by the first harness is a useful
pattern beyond this fixture: each route can project the same named evidence
(payload bytes, mode decisions, CRCs, rejection categories) while an external
driver owns comparison and provenance. A later harness could encode that
projection as bounded NyData with compiler/SDK manifest identity and corpus
digest. It should remain test evidence rather than a second codec API, and it
must never include ambient pixels, private paths, prompts, or model context.

## Open questions

- Should a streaming trace permit independently recoverable frame groups, and
  what exact state may survive a damaged group?
- Which display surface owns pixel normalization before capture when Anomalous
  supports formats beyond RGB8?
- Should frame timing remain rational ticks or bind to an attributable Anomalous
  monotonic timebase identity in a later revision?
- Which context annotations materially improve machine repair without leaking
  private source or making executables too large?
- When real traces exist, do palettes, spatial predictors, or motion search earn
  their complexity under measured ratio and latency?
- Can an Eidolon payload be chunked inside Reliquary without making either
  parser depend on the other's optional features?

## Complete standalone CLI and bounded-codec checkpoint — 2026-09-13

Eidolon now has one clean, manifest-pinned SDK boundary for the complete CLI:

- Ny revision:
  the historical SDK source (not distributed)
- immutable SDK generation:
  `a2e68c24330296811aa0c88ff7a91ff5e985d1dc9b180cf0483f07084e182adf`
- source worktree: clean
- LLVM support: enabled
- `nycompiler`, `nyinterpreter`, `nyassembler`, `nylinker`, the native runtime,
  and `nystd`: verified against that generation's manifest

The full `encode`, `inspect`, and `decode-frame` corpus now runs through five
actual Ny execution routes: textual NIL under `nyinterpreter`, binary NIL under
`nyinterpreter`, LLVM JIT, LLVM AOT, and serialized textual NIL lowered by
`nyassembler` and linked with the SDK runtime by `nylinker`. Python remains only
the independent oracle and assertion driver. It does not perform an operation
attributed to Ny.

`make ny-native-cli` keeps the standalone path inspectable. `make ny-cli`
produces `build/ny/eidolon-cli.text.nil` and
`build/ny/eidolon-cli.binary.nil`; the native target assembles the textual
artifact to the ELF relocatable `build/ny/eidolon-cli.o` and links
`build/ny/eidolon`. Invoking that executable without a command reaches the real
CLI and returns usage status 64. Binary NIL v1 is deliberately not passed to
the assembler: it does not serialize the struct and enum schemas needed to
lower Eidolon safely. It remains a supported interpreter artifact, while
`nyassembler` fails closed at this schema boundary instead of inventing ABI
layouts.

Eidolon also exposed the compiler's generated non-void fallthrough boundary.
The exhaustive `Result` match in `load_regular() -> loaded_file` returns a
struct on every source branch, but the frontend retains a defensive generated
terminator. Ny now emits the explicit reserved marker
`RET $ny.generated.nonvoid-fallthrough`; textual and binary readers preserve
it, and interpreter and native routes trap if it is ever executed. No route
synthesizes a zero, null, or aggregate return value.

The codec and CLI now share `src/limits.ny` as their executable acceptance-limit
authority. Source order places guarded encode checks for geometry, frame size,
tile count, and worst-case payload capacity before later pixel reads and
explicit output allocation. It places decode checks for geometry, frame size,
claimed payload length, and minimum tile-header capacity before later payload
reads and explicit decoded-frame allocation. Low-allocation probes exercise the
expected early status with a one-element backing object and oversized claims.
They support that code-order evidence but are not allocation telemetry and do
not independently measure hidden runtime allocations. The former public
`format::max_*` constants were removed as part of this experimental
centralization; callable `format::maximum_*()` accessors remain and delegate to
`limits.ny`. The 1 MiB logical limits remain conservative acceptance limits,
not wire-format maxima or RSS guarantees. The public PPM parser now checks its
integer input extent against the maximum frame plus the 4096-byte header before
scanning octets; maximum plus one maps to `invalid_raster_length` because no
header bytes have yet been inspected.

The Python and Ny P6 parsers now resolve the CR/CRLF separator ambiguity from
the exact raster length. If consuming CR alone leaves exactly
`width * height * 3` bytes, a following LF is the first raster byte; otherwise
the valid CRLF separator consumes both bytes. Exact vectors cover both a
CR-delimited raster beginning with `0x0a` and a genuine CRLF separator across
the oracle and all five Ny CLI routes.

The scoped validation result is `make test` **61/61**, `make check-ny`
**26/26**, and `make check-ny-native` **6/6**. This closes the bounded complete
CLI/toolchain checkpoint. It does not close streaming I/O and transactional
multi-frame publication, byte-specialized storage and allocation accounting,
SIMD or inline-assembly acceleration with CPU gating, long-running sanitizer
and fuzz coverage, or compression-ratio, latency, and mode-histogram evidence
from a named real Anomalous capture corpus.
