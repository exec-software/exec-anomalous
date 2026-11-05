# Ny feature ledger for Eidolon

## Purpose

Eidolon is a production-shaped language proving ground, not a syntax showcase.
Every Ny feature below has a codec responsibility, an observable oracle, and a
route-specific gate. Features that cannot yet meet the contract remain blocked
rather than being replaced with unchecked casts or no-op placeholders.

This ledger began with the toolchain state observed on 2026-09-09 and includes
the interpreted and clean-SDK native complete-file observations from
2026-09-13. It is not a permanent statement about Ny; update it only with a
reproducer and evidence.

| Capability | Eidolon responsibility | Current state | Completion evidence |
| --- | --- | --- | --- |
| Fixed-width octets | Headers, pixels, CRC input, exact file bytes | File boundaries use exact `array<u8>` on both NIL-interpreter routes, LLVM JIT/AOT, and the standalone textual assembler/linker route; the portable codec deliberately retains range-checked `i32` logical octets and `ptr<i32>` scratch buffers, and complete containers and PPMs match the oracle byte-for-byte on all five CLI routes | Replace internal logical octets with a proved compact representation and add randomized ABI-boundary differentials |
| Checked integer arithmetic | Geometry, offsets, allocation budgets | `limits.ny` is the single executable authority for the public codec and CLI. The complete-container path bounds the entire PPM input before its octet scan, then bounds headers, geometry, frame bytes, tile count, aggregate decoded bytes, payloads, container bytes, offsets, and runs before use on all five CLI routes; each logical-byte budget is 1 MiB, but these checks do not measure VM RSS | Expand overflow and boundary vectors and back logical budgets with accurate VM allocation accounting |
| Arrays and indexing | Frame and tile buffers | All five CLI routes reproduce complete multi-frame traces and canonical PPMs using `array<u8>` file data; the textual/binary NIL and LLVM JIT/AOT in-memory tile routes also cover word arrays | Add randomized array differentials, compact internal pixel storage, and generic bounded buffers |
| Bitwise operators | CRC32C and XOR prediction | CRC32C and XOR/pixel modes match the oracle on all five complete CLI routes and on the four in-memory tile routes | Add randomized differential testing and the independently guarded accelerated path |
| Structs and enums | File/frame headers, modes, typed failures | Complete records and the tile-choice/result structs pass the bounded interpreter, JIT/AOT, and textual assembler/linker corpora; native aggregate ownership is released with the runtime context at process exit | Add schemas to a future binary NIL revision, and exercise repeated operations in one long-running process under sanitizer and leak checks with no representation-dependent casts |
| Results and propagation | Recoverable parser and I/O failure | All five complete CLI routes match secure-I/O `Result` values and map pure codec/parser status enums to path-redacted process failures; malformed-input and publication cases leave no partial output, while general `?` lowering is still not used or claimed | Extend malformed-vector categories and adopt proved propagation syntax without weakening explicit status mapping |
| Byte file I/O | PPM and `.eidolon` streaming | Complete for bounded whole-file use on all five CLI routes: no-follow regular reads reject special files and growth, atomic writes reject symlink/reparse points, encode replaces one container, and decode publishes one new PPM | Add fault injection for partial write, flush, and close failures, then implement bounded streaming and a multi-output transaction |
| Process arguments | Safe CLI paths and options | All five CLI routes accept exact argument vectors for `encode`, `inspect`, and `decode-frame`; native harnesses pass explicit argv, including paths containing spaces, without shell reparsing | Expand adversarial argument coverage and establish the supported-platform argv contract |
| Pointers and allocation | Bounded scratch buffers and native performance | Word-pointer allocation and indexed access match the tile oracle on textual/binary NIL and LLVM JIT/AOT, and the complete CLI passes all five routes with runtime-context ownership cleanup | Exercise repeated allocations and complete file operations under sanitizers and leak checking where available |
| Inline assembly | Optional accelerated CRC or transforms | Blocked safely: syntax is recognized, but compilation now rejects it because no backend lowering exists | Typed operands/clobbers, architecture guard, CPUID dispatch, disassembly inspection, and portable-oracle equivalence |
| Modules and generics | Separated codec layers and reusable bounded buffers | The strict CLI composes `cli.ny`, `format.ny`, `ppm.ny`, `codec.ny`, and `limits.ny` with active SDK modules on all five CLI routes. The experimental API centralizes limit values in `limits.ny`; former public `format::max_*` constants are removed while callable `format::maximum_*()` accessors remain. Generic bounded buffers are not yet used | Binary-library consumption and a generic bounded-buffer abstraction work without hidden fallback |
| AI annotations | Intent, invariants, test links, governed context | Parsed in current examples; executable embedding and disclosure contract remain under review | Manifest/executable inspection proves exact retained metadata, size bounds, independent trust, and opt-in disclosure |
| Runtime logging | Attributable phase, limit, and failure records | CLI success is stable key/value output and failures contain operation, stage, and numeric status without paths or pixels; this bounded projection is not yet the structured runtime-logging design | Versioned structured records agree for human and machine clients and carry trace attribution without private payloads |

## Route matrix

The Python oracle first encodes, decodes, and rejects the committed vectors.
The complete Ny CLI must then match that evidence through five executable
routes:

1. checked textual NIL under `nyinterpreter`;
2. checked binary NIL under `nyinterpreter`;
3. LLVM JIT;
4. LLVM AOT; and
5. serialized textual NIL lowered by `nyassembler` and linked by `nylinker`.

Binary NIL v1 is not substituted into the fifth route. It does not serialize
the struct and enum schemas required to lower Eidolon safely, so `nyassembler`
rejects it. Textual NIL is the lossless standalone interchange until a future
binary revision carries those schemas. An optional CPU-gated SIMD or inline-
assembly path comes only after it matches the portable Ny path for every vector
and randomized corpus.

A passing parser or successful code generation does not substitute for runtime
behavior. A passing interpreter does not imply that the native route works.

## Evidence recorded 2026-09-09

The first Ny vertical slice is now observed working. The harness compiles
`src/main.ny` plus `src/codec.ny` without the standard library and without
`--no-typecheck`, once as textual NIL and once as binary NIL, and executes both
with `nyinterpreter`. Six Python differential tests establish:

- exact CRC32C check value `0xe3069283` and exact key/delta payload CRCs;
- byte-for-byte key payload equality for raw and solid edge tiles;
- byte-for-byte delta payload equality for unchanged and XOR-RLE tiles;
- exact pixel-RLE record bytes and deterministic tie behavior;
- exact reconstruction of both 5-by-3 oracle frames and the RLE vector; and
- structured rejection of short raw data, zero and overflowing runs, an
  unknown mode, a trailing byte, and a temporal mode in a key frame.

Run the evidence from `eidolon/` with `make check-ny` after `make sdk`, or set
`NY_COMPILER`, `NY_INTERPRETER`, and `NY_STDLIB` to one checked Ny build and
source tree when developing the two repositories together. Explicit overrides
fail closed when missing or unusable, and the harness prints the resolved tool
paths and executable SHA-256 values. It attempts LLVM JIT and AOT and skips
those two named tests only when the compiler emits its explicit LLVM-disabled
diagnostic.

## Native in-memory tile evidence observed 2026-09-13

An LLVM-capable Ny build now runs `src/main.ny` plus `src/codec.ny` through JIT
and AOT. Both native routes produce the same parsed evidence map as textual and
binary NIL: CRCs, payload bytes, mode choices, decoded pixels, and malformed
tile statuses all match. This exercises native structs, enums, `array<i32>`,
`ptr<i32>`, allocation, indexing, and bitwise work in one bounded application.

This was the first native boundary, preceding the complete-CLI evidence below.
The later clean-SDK gate now covers byte-specialized arrays, secure byte-file
intrinsics, arguments, complete `format.ny` records, and PPM publication. The
remaining ownership test is repeated work in one long-running process under
sanitizers; there is no longer a known returned-struct lifetime blocker in this
bounded process-per-command route.

## Complete file-path milestone observed 2026-09-13

The next vertical slice now parses real bounded P6 inputs, deterministically
encodes complete file and frame records, validates complete hostile containers,
reports inspect metadata and mode counts, selects any verified frame, and
atomically publishes one canonical P6 output. The CLI commands are `encode`,
`inspect`, and `decode-frame`; the last deliberately publishes one file per
invocation because Ny does not yet expose an atomic directory transaction.

The integration corpus compiles the five-module CLI with strict checking and
exercises it through textual and binary NIL interpretation, LLVM JIT/AOT, and
standalone textual NIL assembly and linking. It establishes byte-identical
multi-frame encode against the Python oracle, exact decoded PPMs for every
frame, exact mode histograms, full-width unsigned timestamp/timing validation,
a shared inclusive 4096-byte PPM-header boundary, and matching rejection of
file CRC, payload CRC, unknown mode, decoded CRC, timestamp regression,
zero/max geometry and frame counts, file/frame flags, zero duration,
uncompressed and maximum payload sizes, truncated headers, and trailing-data
mutations. P6 vectors distinguish both valid spellings at the ambiguous
boundary: with an exact known raster length, CR followed by a raster-leading LF
consumes only CR, while a genuine CRLF separator consumes both bytes. Security
cases prove nonblocking FIFO rejection, no-follow input, path-redacted failures,
no-replace output including dangling symlinks, and refusal to replace an output
symlink even when encode otherwise permits replacement. The Python publication
oracle additionally proves exact POSIX mode-0600 files and a mode-0700 decoded
directory under `umask 0777`, with no ordinary-path staging orphan.

Two compiler defects appeared and were fixed in the same Ny iteration.
`while offset < item.finish` in the PPM decimal loop was parsed as a generic
argument list and reported `Expected '>' after generic arguments`. A bounded
type-lookahead fix now accepts the natural comparison and the full suite passes
with it restored. Separately, same-named helpers in different modules were
resolved to the wrong function in emitted NIL even when source-qualified. Ny's
module-aware lookup was repaired and its exact textual and binary NIL reproducer
now passes. Eidolon retains the clearer unique `trace_status_code`,
`ppm_status_code`, and `failed_trace_*` names by choice, not as an application
blocker. Both defects have minimal reproducers in the integration notebook and
belong in Ny compiler regression tests rather than being normalized as language
behavior.

Eidolon later exposed a third compiler boundary in
`load_regular() -> loaded_file`: its exhaustive `Result` match returns a struct
from every source branch, while the frontend still emits a defensive
fallthrough terminator. Ny now marks that generated impossible edge explicitly
as `RET $ny.generated.nonvoid-fallthrough`. Textual and binary NIL readers
preserve the marker, and interpreter and native execution trap if control ever
reaches it; no route invents a zero, null, or aggregate return value. This keeps
a real non-void fallthrough fail-closed while allowing exhaustive generated
control flow to lower consistently.

## Interpreter memory profile observed 2026-09-13

GNU `/usr/bin/time -v` measured the textual-NIL CLI on patterned RGB8 inputs:

| Operation | Logical input/output | Maximum RSS |
| --- | ---: | ---: |
| Encode 320×200, one frame | 192,000-byte raster; 193,392-byte container | 119,384 KiB |
| Encode 640×480, one frame | 921,600-byte raster; 927,692-byte container | 477,748 KiB |
| Inspect that 640×480 trace | 927,692-byte container | 477,200 KiB |
| Decode that 640×480 trace | 921,615-byte PPM | 624,612 KiB |

The prior 16/32/64 MiB logical limits could therefore imply unsafe multi-GiB
resident memory. The shared `limits.ny` profile now caps frame, payload,
container, and aggregate decoded logical bytes at 1 MiB each for both the
public codec API and CLI. Source order places guarded encode preflights for
geometry, frame size, tile count, and worst-case payload capacity before later
pixel scans and explicit output allocation. It likewise places decode guards
for geometry, frame size, claimed payload length, and minimum tile-header bytes
before later payload scans and explicit decoded-frame allocation. Low-allocation
probes pass a one-element backing object with oversized claims and exercise the
expected early statuses on each route. This combined code and input evidence
supports the ordering; the status alone is not allocation telemetry and does
not measure hidden runtime allocations. The former public `format::max_*`
constants were removed as part of this experimental centralization, while the
callable `format::maximum_*()` accessors remain and delegate to `limits.ny`.
The profile deliberately retains one practical 640×480 default-tile
framebuffer case, but it is not an RSS bound or a production-size promise. Both
NIL routes reject a 1 MiB plus one byte container at the bounded read before
decoding. Ny needs byte-specialized array storage, fewer transient whole-buffer
copies, and accurate VM allocation accounting before this profile can be
raised responsibly.

## Deliberate sequencing

The portable path now spans CRC32C, all draft-1 tile modes, complete records,
strict PPM, hostile decode, arguments, and secure whole-file I/O on all five
complete CLI routes. Streaming, a multi-output transaction, long-running
sanitizer coverage, real Anomalous corpus measurements, and optional
SIMD/inline assembly still follow this portable oracle; they are not inferred
from bounded process success.

## Complete toolchain and CLI gate passed 2026-09-13

The integration harness resolves the root SDK tools `nycompiler`,
`nyinterpreter`, `nyassembler`, and `nylinker`, plus the native runtime and
`nystd`, from one immutable SDK generation. It verifies those artifacts and the
complete source tree against the manifest before attributing a result to Ny.
The same SDK root packages exactly eight public tools: those four plus `nylsp`,
`nydebugger`, `nyschema`, and `nyagent`.

LLVM AOT compiles the real five-module program (`cli.ny`, `format.ny`,
`ppm.ny`, `codec.ny`, and `limits.ny`) once and invokes the resulting executable
with an argv vector. LLVM JIT invokes the same program through
`nycompiler --native --jit`, with `--` separating compiler inputs from explicit
application arguments. The standalone route first emits inspectable textual
NIL, lowers it with `nyassembler` to `build/ny/eidolon-cli.o`, and links that
ELF relocatable with the SDK runtime through `nylinker` to
`build/ny/eidolon`. `make ny-native-cli` produces both artifacts; invoking the
executable without a command reaches the CLI and returns its documented usage
exit status 64.

The textual-NIL interpreter, binary-NIL interpreter, JIT, AOT, and standalone
textual assembler/linker routes all exercise `encode`, `inspect`, and
`decode-frame`; compare container and canonical P6 bytes with the Python
oracle; check inspect metadata; and require malformed PPM, corrupt-container,
existing-output, and POSIX symlink failures to leave destinations absent or
byte-identical. Python supplies expected bytes and metadata only. It never
performs the operation attributed to Ny.

The validated checkpoint passes `make test` **61/61**, `make check-ny`
**26/26**, and `make check-ny-native` **6/6**. The pinned evidence is:

- Ny source revision: the historical SDK source (not distributed)
- SDK generation: `a2e68c24330296811aa0c88ff7a91ff5e985d1dc9b180cf0483f07084e182adf`
- source worktree: clean
- LLVM support: enabled
- `nycompiler` SHA-256: `8b0617c59f711cdc8c9790eec0506ee374b1f20b84e82b131461ff1540cd26e7`
- `nyinterpreter` SHA-256: `36f2796f3b139532d365528d18b82a2a13951eee50013e67efcc2ad3274474c9`
- `nyassembler` SHA-256: `2ce84c1a469e7e64b13b8e586f19beb4516c9fb33fa005ab4892fd1b59ca5141`
- `nylinker` SHA-256: `d98382db13a473d725816bd8475a5eb6c83a85c6f85a31902545214b0e2e3343`
- `nylsp` SHA-256: `2adb445271f1c3b6686e3f03942c6c85f386e2b3a8e0dfad3e0e22375fdffa42`
- `nydebugger` SHA-256: `91ce80e2a50c494da5b1c5379aa1b800d6c7675f4f1c603d91f06b830ebd0d5c`
- `nyschema` SHA-256: `3649e5681c537cb4e42362ec9a53b07eab117317679b486ec71883573bdadf0c`
- `nyagent` SHA-256: `99a81708a595590632eebd6d2d0fc48a8d1052587eaa918141f64d5275652cd8`
- native runtime SHA-256: `1fceffb6dd15743cd1f53561336fdb5c1a4d5288f206dd3461158e4f0c1a3e2d`
- aggregate `nystd` SHA-256: `516850981b6619698cce09d6adf8b4f06d5d455fa21b93992689c58d9cdae21d`

Binary NIL v1 remains supported by `nyinterpreter`, but its reserved type and
symbol sections do not carry the struct and enum schemas required by Eidolon.
`nyassembler` therefore rejects the binary artifact rather than guessing ABI
layouts; only the lossless textual artifact feeds the standalone native route.
This boundary does not turn whole-file operation into streaming, prove
cross-platform native I/O, establish SIMD/inline assembly, or provide real
Anomalous corpus ratio and latency measurements.

Language features without a natural codec role belong in focused Ny
conformance applications. They should not be forced into Eidolon merely to
claim coverage; the aggregate project test ledger can link those results.
