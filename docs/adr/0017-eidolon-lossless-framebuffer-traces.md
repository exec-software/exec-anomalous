# ADR 0017: Build Eidolon lossless framebuffer traces

- Status: Proposed
- Date: 2026-09-09

This record proposes a codec, name, and experimental byte format. None is a
stable compatibility promise while the record remains Proposed.

## Context

Anomalous already exposes a packed-pixel framebuffer during bring-up, and QEMU
can capture that surface as P6 PPM images. Raw frame sequences provide exact
visual evidence but are expensive to retain: the current 1024 by 768 RGB8
surface is 2,359,296 bytes per frame, or about 141.6 MB/s at 60 frames per
second before framing.

A lossless framebuffer trace is useful for regression evidence, renderer and
compositor debugging, remote presentation research, deterministic recovery
artifacts, and later Oracle correlation. It also provides a serious Ny workload
with externally observable requirements across bytes, bitwise operations,
checked arithmetic, temporal state, I/O, pointers, diagnostics, and native
optimization.

Existing image and video codecs either lose exact pixels, carry a much wider
media and dependency surface, or do not exercise the Ny systems boundaries the
project needs to prove. Defining a new parser also creates risk: framebuffer
pixels are sensitive and compressed lengths are hostile allocation claims.

## Decision

### Name and scope

Name the proposed lossless framebuffer-trace codec and container **Eidolon**.
On first external use, describe it conventionally as a lossless framebuffer
trace. The name has only the technical meaning recorded in
[the nomenclature](../nomenclature.md).

Place the project at repository root under `eidolon/`. Its immediate input is a
sequence of equally sized packed RGB8 P6 PPM captures, including QEMU
`screendump` output. Its output conventionally uses `.eidolon` and the proposed
media type `application/vnd.anomalous.eidolon`.

Eidolon is host-side development tooling. It is not linked into Threshold or
the kernel. A future implementation runs in an unprivileged codec or display
vessel with explicit resource budgets. Rich container parsing never belongs in
an IRQ, syscall fast path, kernel recovery path, or boot loader.

### Experimental draft 1

Use a byte-exact little-endian draft with:

- a fixed magic, version, RGB8 geometry, tile geometry, rational timebase,
  frame count, and header CRC32C;
- sequential key or delta frame records with checked index, timestamp,
  duration, encoded and decoded sizes, encoded CRC32C, decoded CRC32C, and
  frame-header CRC32C;
- clipped raster-order tiles using deterministic raw, solid, pixel RLE,
  unchanged, or XOR-byte-RLE modes; and
- no implicit padding, metadata, dynamic geometry, audio, unknown extension,
  or trailing data.

The exact draft is documented in
[`eidolon/docs/format-draft-1.md`](../../eidolon/docs/format-draft-1.md) and
implemented by a dependency-free Python oracle. Frame zero is a key frame.
Delta frames refer only to the preceding fully verified frame. A decoder never
publishes a candidate frame until both its payload and complete decoded pixel
CRC pass.

Encoding is deterministic. Each tile selects the shortest available payload,
with a fixed tie order. Same pixels, geometry, timing, tile size, and keyframe
interval produce byte-identical output.

CRC32C is accidental-corruption evidence, not authentication. Draft files may
be replaced incompatibly until an acceptance ADR freezes a revision and its
conformance corpus.

### Ny proving path

The Python implementation is the independent behavioral oracle, not the final
product implementation. Ny proceeds in evidence-gated slices:

1. portable in-memory CRC32C and key-frame tile encode/decode;
2. delta frames with explicit previous-frame ownership;
3. checked byte streams and PPM/file CLI;
4. interpreted NIL and native cross-conformance over the same corpus; and
5. optional CPU-gated optimized routines after portable equivalence.

The live status and blockers are recorded in
[`eidolon/docs/ny-feature-ledger.md`](../../eidolon/docs/ny-feature-ledger.md).
No unchecked cast, placeholder intrinsic, `--no-typecheck`, or no-op assembly
path counts as codec evidence.

Eidolon consumes the exact project-wide SDK defined by ADR 0016. SDK refresh
must not partially replace an active toolchain.

### Capture, privacy, and system integration

Framebuffer pixels can expose credentials, messages, source, personal media,
prompts, and model output. The codec performs no ambient capture and receives
only caller-selected frames. It does not silently collect input events, command
text, window titles, accessibility text, clipboard contents, model context,
object names, or authority.

A future system capture path is a typed rite shared by human and machine
clients. It resolves a bounded surface, frame/time budget, destination,
principal, purpose, retention, and disclosure policy before capture and
requires a narrow sigil. Access to the codec itself grants no capture authority.

Eidolon traces do not enter Oracle telemetry automatically. Oracle may later
retain only explicitly policy-approved artifacts and privacy-safe structural
diagnostics. Failures identify the stage, frame, bound, and check without
logging pixels or private paths by default.

A future Eidolon payload may be a chunk inside a signed, governed Reliquary.
Eidolon does not compete with Reliquary as the general executable, package,
provenance, signature, or context container.

## Required invariants

- Every dimension, multiplication, count, length, offset, run, and allocation
  is checked against an explicit local budget before use.
- Unknown draft-1 flags, modes, sizes, versions, and trailing data fail closed.
- Tile records fill their exact geometry; RLE runs are nonzero and neither
  underfill nor overfill.
- A delta tile reads only the prior complete verified frame.
- A failed candidate never replaces or becomes observable as a verified frame.
- Identical encoder inputs and options produce identical bytes.
- Parser errors are attributable without emitting pixel contents.
- Capture policy remains outside the codec and is identical for human and
  machine clients.
- No privileged Anomalous component parses the rich format.
- Optimized Ny or assembly paths remain bit-exact with the portable oracle and
  require explicit architecture detection.

## Non-goals

This proposal does not define:

- lossy compression, photographic quality tuning, audio, subtitles, color
  management, HDR, alpha, dynamic resolution, or general video interchange;
- a stable wire, network-streaming, random-access, or archive format;
- kernel capture, kernel decompression, or direct device DMA;
- input recording, interaction replay, accessibility capture, or user identity;
- authentication, encryption, signatures, provenance, or retention policy;
- a replacement for Reliquary, Oracle, or a framebuffer familiar; or
- production performance or compression claims.

Motion search, palette modes, entropy coding, indexing, streaming recovery, and
hardware acceleration remain later experiments measured against real traces.

## Acceptance evidence

The proposal may be accepted only after:

- committed golden vectors and exact draft documentation agree;
- the dependency-free oracle passes round-trip, deterministic, edge-tile,
  truncation, corruption, unknown-mode, RLE-bound, trailing-data, and resource-
  budget tests;
- Ny interpreter and native routes cross-encode and cross-decode every vector
  byte-for-byte against the oracle;
- mutation or coverage-guided fuzzing runs under explicit time and memory
  bounds with no uncontrolled allocation or partial-frame publication;
- QEMU PPM traces encode and decode to exact pixel hashes;
- measurements report named corpus, host, route, size, ratio, throughput, peak
  memory, and frame latency; and
- capture authority and privacy receive a separate review before any live
  Anomalous integration.

The current Python oracle is useful implementation evidence but is not enough
to accept this ADR or claim a stable format.

## Consequences

The project gains compact exact visual traces, an independent Ny oracle, and a
real workload that exposes incomplete language behavior. Deterministic small
modes are straightforward to inspect and test.

It also gains another hostile parser and experimental format to maintain.
Lossless RGB capture can still consume substantial CPU, memory, and storage and
can preserve secrets perfectly. Strict budgets, user-space isolation, explicit
capture policy, and refusal to promise compatibility are therefore part of the
design rather than later polish.

## Alternatives considered

### Keep only raw PPM sequences

This is simple and exact but quickly becomes expensive to retain and does not
exercise temporal, framing, bitwise, or robust decode behavior in Ny.

### Adopt an existing general video codec immediately

Mature codecs provide interoperability and performance, but a full codec stack
adds dependencies and attack surface while obscuring the Ny language gaps this
project is meant to reveal. Eidolon remains narrow and experimental; existing
codecs can be added later in a media vessel.

### Put traces directly in Reliquary

Reliquary can govern and sign a codec payload, but it should not define pixel
prediction or tile decoding. Keeping those layers separate preserves their
different parsing, evolution, and policy responsibilities.

### Capture in the kernel for speed

This would put rich policy and data movement in the most privileged boundary.
A bounded user-space surface grant and shared memory can later avoid copies
without moving the codec into ring 0.
