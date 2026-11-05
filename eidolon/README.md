# Eidolon

Eidolon is an experimental lossless framebuffer-trace codec for Anomalous. It
is a real workload for bringing Ny to production readiness: exact byte layout,
checked arithmetic, bitwise CRC32C, tile prediction, temporal state, file I/O,
pointers, native compilation, and eventually constrained SIMD assembly all
have observable correctness criteria here.

The dependency-free Python implementation remains the independent reference
oracle. Ny now implements the complete bounded draft-1 data path:
`src/codec.ny` owns tile coding, `src/format.ny` owns file and frame records,
`src/ppm.ny` owns strict P6 parsing and emission, `src/limits.ny` is the shared
acceptance-limit authority, and `src/cli.ny` connects bounded regular-file
reads and atomic output to real process arguments. The sources compile without
`--no-typecheck`. One oracle corpus validates the complete CLI through textual
and binary NIL under `nyinterpreter`, LLVM JIT, LLVM AOT, and a standalone
textual-NIL-to-`nyassembler`-to-`nylinker` executable.

This is a complete bounded portable milestone, not yet the final product
codec. The implementation still uses checked `i32` values as logical octets
inside the codec, with exact `array<u8>` conversion at file boundaries. The
shared profile caps each frame, payload, container, and total decoded output at
1 MiB. Public codec entry points have guarded early preflights for impossible
geometry, payload capacity, and minimum tile-header claims, and the public PPM
parser rejects an oversized input extent before scanning its octets. Source
order places those guards before later pointer scans and explicit result
allocation. One-element low-allocation inputs exercise the guarded status
paths; these tests are not allocator telemetry. The experimental API removes the former
public `format::max_*` constants and centralizes their values in `limits.ny`;
the callable `format::maximum_*()` accessors remain. These are logical-byte
acceptance limits, not RSS limits: generic VM values and transient arrays make
current memory use much larger. The profile still accepts one 640-by-480 RGB8
frame with default tiles. Strict P6 parsing
uses the exact raster length to distinguish a CR separator followed by a
raster-leading LF from a genuine CRLF separator. A multi-output atomic
directory transaction, streaming handles, real Anomalous corpus measurements,
and a CPU-gated SIMD/inline-assembly path remain open. The
[feature ledger](docs/ny-feature-ledger.md) records the measurements and the
evidence needed to close those boundaries.

Eidolon is host-side development tooling today. It is not linked into
Threshold or the kernel, and no current Anomalous boot path parses the format.

## Runtime compatibility

The commands below retain a historical SDK contract. See
[optional runtime requirements](../docs/optional-runtime.md) before running them.
Compatibility with the current public NIL framework is not qualified. The
language sources are preserved for research and porting; no SDK is bundled.

## Try the reference codec

QEMU's monitor can produce binary P6 PPM screenshots with `screendump`. Given a
sequence of equally sized screenshots:

```sh
cd eidolon
make test-reference
python3 -m reference.cli encode trace.eidolon frame-000.ppm frame-001.ppm
python3 -m reference.cli inspect trace.eidolon
python3 -m reference.cli decode trace.eidolon decoded-frames
cmp frame-000.ppm decoded-frames/frame-000000.ppm
```

Encoding is deterministic for identical pixels, geometry, timing, tile size,
and keyframe interval. Decoding checks every header, length, mode, CRC, resource
budget, frame index, and trailing byte before returning a trace. See the
[draft format](docs/format-draft-1.md) and
[security and privacy contract](docs/security-and-privacy.md).

Refresh the project SDK before running the Ny or complete test suites, then
exercise the Ny oracle comparison as well:

```sh
make sdk
make check-ny
make check-ny-native
make test
```

The current checkpoint passes `make test` **61/61**, `make check-ny` **26/26**,
and `make check-ny-native` **6/6**. The harness invokes the SDK tools directly.
It never uses Python to produce a result attributed to Ny; Python only computes
independent expected bytes and checks the Ny transcript and artifacts. Native
tests report skipped only when `nycompiler` explicitly identifies an
LLVM-disabled build.

Build both inspectable Ny CLI routes and invoke either artifact directly:

```sh
make ny-cli
sdk/nyinterpreter --no-ai build/ny/eidolon-cli.text.nil -- \
    encode trace.eidolon frame-000.ppm frame-001.ppm
sdk/nyinterpreter --no-ai build/ny/eidolon-cli.binary.nil -- \
    inspect trace.eidolon
sdk/nyinterpreter --no-ai build/ny/eidolon-cli.binary.nil -- \
    decode-frame trace.eidolon 1 frame-000001.ppm
```

Build the standalone native CLI from inspectable textual NIL and invoke the
resulting artifact directly:

```sh
make ny-native-cli
build/ny/eidolon inspect trace.eidolon
```

`make ny-native-cli` lowers `build/ny/eidolon-cli.text.nil` with
`nyassembler`, then links the host object and SDK runtime with `nylinker`.
Binary NIL v1 remains a valid `nyinterpreter` input, but it intentionally is not
an assembler input for Eidolon: that format does not serialize the struct and
enum schemas needed for safe native lowering. The assembler fails closed
instead of inventing layouts. Textual NIL is the lossless standalone route.

`encode` validates every input before atomically replacing one output
container. `inspect` is read-only. `decode-frame` validates the complete trace
before atomically publishing one requested canonical P6 file and refuses any
pre-existing output entry. Selecting one output per invocation is intentional:
Ny does not yet expose the directory transaction needed to promise all-or-none
publication of many PPM files.

## Shared Ny runtime

Eidolon compiles and runs Ny through Anomalous's single, project-wide runtime at
the repository root, `runtime/`. The same runtime is shared by every
sub-project, including `oculus/`, so Matrix and Ny sources build against one
compiler and one standard library.

Select a compatible historical SDK source checkout explicitly:

```sh
NY_REPO_DIR=/path/to/compatible-sdk-source make sdk
```

`make sdk` runs `../scripts/refresh-ny-runtime.sh`. The builder stages an
immutable generation under `.runtime/store/versions/`, activates it through the
builder's managed symlink at `.runtime/sdk`, then copies the resolved generation
into a real `runtime/` directory so no sub-project depends on a symlink into
another checkout. `runtime/` and `.runtime/` are both ignored by Git.

Historical validation used a marked development SDK. The refresh helper now
requires explicit source selection and defaults to clean source. Its manifest
records source identity, dirty state, and file digests; a development SDK is not
a language release declaration.

Override the checkout with
`NY_REPO_DIR=/path/to/compatible-sdk-source make sdk`, and set
`NY_RUN_TESTS=OFF` to skip the Ny test suite for a faster development rebuild.

The activated runtime contract is exact:

```text
runtime/
├── nycompiler
├── nyinterpreter
├── nyassembler
├── nylinker
├── nylsp
├── nydebugger
├── nyschema
├── nyagent
├── nylib/          compiled standard-library .nil modules
├── nystd/          standard-library .ny source
├── docs/           compiled HTML documentation
├── manifest.json
└── LICENSE
```

There is no `bin/` directory and no retired runtime compatibility alias. Runtime
creation is strict: if any standard-library source fails checked compilation, no
incomplete generation becomes active.

## Milestones

1. Keep the Python oracle and golden vector deterministic and hostile-input
   tested. **Active and checked.**
2. Implement an in-memory Ny encoder and decoder and cross-check every result
   against the oracle through `nyinterpreter`. **The tile-payload slice now
   passes textual NIL, binary NIL, LLVM JIT, and LLVM AOT against one parsed
   oracle.**
3. Extend the Ny slice through complete file/frame records and a PPM/file CLI.
   **Complete for textual and binary interpreted NIL, LLVM JIT/AOT, and the
   standalone textual NIL assembler/linker route, with bounded no-follow input
   and atomic single-file output. Generated non-void defensive fallthrough is
   explicit in NIL and traps if reached instead of synthesizing a value.**
4. Measure the existing unchanged/XOR temporal modes on Anomalous traces, then
   add predictors only where named-corpus evidence justifies their complexity.
5. Add an optional, CPU-gated optimized CRC or transform only after the portable
   Ny implementation remains the conformance authority.

Draft-1 `.eidolon` files are experimental test artifacts. They are not a stable
compatibility promise and should not contain sensitive framebuffer data.
