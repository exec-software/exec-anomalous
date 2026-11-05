# Eidolon security and privacy contract

## Boundary

An Eidolon file is hostile rich input. Parsing belongs in a constrained
host-side tool today and eventually in an unprivileged vessel. Threshold, the
kernel, an interrupt handler, or another privileged recovery path must never
parse this container.

The reference decoder checks its complete input before returning frames. It
uses checked geometry arithmetic, explicit allocation budgets, exact record
counts, length-before-slice validation, known-mode enforcement, CRC32C over
headers and payloads, decoded-frame CRC32C, contiguous frame indexes, and
trailing-data rejection. Tests mutate framing, payloads, modes, runs, lengths,
and resource limits.

The reference CLI accepts only regular PPM and container input files. On POSIX,
reads open the final path component with `O_NOFOLLOW`, verify the opened file's
type and size, read exactly the initial descriptor size, require immediate EOF,
and compare device, inode, size, modification time, and change time after the
read. This rejects final-component symlinks, short reads, growth, and observed
same-size mutation without trusting a prior path lookup. Python's standard
library does not expose the corresponding Windows reparse-point open flag, so
the Python oracle retains exact-read and post-metadata checks there but cannot
claim the POSIX no-follow guarantee. The Ny runtime has a native Windows
reparse-point implementation.

The Python oracle's 1 GiB default container limit is likewise a logical input
budget, not a resident-memory guarantee. Its exact snapshot read currently
materializes a `bytearray` and then an immutable `bytes`, briefly duplicating
the input before decode allocations; a near-limit file can exhaust memory.
Production callers must use a measured lower limit or a future streaming
reader rather than treating the oracle default as safe allocation authority.

The Ny CLI uses the stricter SDK `read_regular_bytes(path, maximum_bytes)`
boundary on interpreted, JIT/AOT, and standalone native routes. It opens once in
nonblocking/no-follow mode, verifies the opened descriptor is a regular file,
and reads at most the caller's limit plus one growth-detection byte. It rejects
FIFO/device input and every observed symlink or reparse point. Runtime errors
and the CLI projection omit caller paths. The portable codec stores logical
octets in generic VM value slots and creates transient copies. Its empirically
conservative profile in `src/limits.ny` therefore caps each frame, payload,
container, and total decoded output at 1 MiB for both public codec calls and the
CLI. Source order places guarded encode checks for geometry, frame size, tile
count, and worst-case payload capacity before later pixel scans and explicit
output allocation. It similarly places decode checks for geometry, frame size,
claimed payload length, and minimum tile-header bytes before later payload
scans and explicit frame allocation. Low-allocation conformance probes exercise
the expected early statuses with a one-element backing object and oversized
claims. This is code-order and guarded-input evidence, not allocator telemetry;
the status alone does not measure or exclude hidden runtime allocations. The
public P6 parser separately rejects an input extent above the maximum frame plus
the bounded 4096-byte header before scanning caller-provided octets. These
logical-byte checks do not bound RSS.
On the 2026-09-13 development build, a patterned 640-by-480 single-frame trace
used 477,748 KiB peak RSS to encode, 477,200 KiB to inspect, and 624,612 KiB to
decode to PPM. A byte-specialized array representation and accurate VM
allocation accounting remain required before larger production limits can be
claimed safely.

Strict P6 parsing accepts the draft's single whitespace separator and its CRLF
spelling without treating an image byte as header whitespace. Once width and
height establish the exact RGB8 raster length, a CR followed by LF consumes
only CR when the remaining bytes are exactly one raster; the LF is then the
first pixel byte. Otherwise the valid CRLF pair is consumed together. The same
rule is implemented by the Python oracle and Ny parser, under the same bounded
header scan, and is exercised on every executable route.

Serialized native lowering also fails closed at two metadata boundaries. Binary
NIL v1 remains executable by `nyinterpreter`, but it does not serialize the
struct and enum schemas Eidolon requires, so `nyassembler` rejects it rather
than guessing layouts; standalone builds consume lossless textual NIL. A
compiler-generated defensive fallthrough in a non-void function is represented
as `RET $ny.generated.nonvoid-fallthrough`, preserved by both NIL readers, and
traps in interpreter or native execution if reached instead of returning a
synthetic zero, null, or aggregate value.

These checks reduce parser and accidental-corruption risk. Python memory safety
does not establish production isolation, and CRC32C provides no authenticity.
Signed provenance, if needed, belongs in a governed outer artifact such as a
future Reliquary rather than in an ad hoc codec flag.

## Capture policy

Framebuffer pixels may contain credentials, private messages, source code,
personal images, protected media, prompts, model output, or other sensitive
context. A valid image is not automatically safe to retain or transmit.

Eidolon therefore has no ambient capture behavior. A caller must explicitly
select each input frame and output path. The codec does not automatically
collect keyboard or pointer input, command text, window titles, accessibility
text, clipboard data, prompts, model context, object identities, or sigils.
Adding any of those would require a separate typed schema, disclosure policy,
retention rule, and architectural review; it must not be smuggled in as codec
metadata.

Current traces stay local development artifacts and must not enter Oracle
telemetry automatically. A future framebuffer familiar or compositor may
provide an explicitly bounded read-only capture surface under a narrow sigil.
The Oracle may receive only policy-approved artifacts with principal, purpose,
retention, and redaction decisions made outside the codec.

Human and machine clients must request the same capture rite and see the same
scope and effects. A machine client cannot infer capture authority from access
to Ny, the codec library, or an output directory.

## Decode and publication

A future streaming decoder needs two complete frame buffers or equivalent
transactional storage: a previously verified reference and a candidate output.
It must not expose a candidate surface to a renderer, consumer, or shared
memory recipient until all tile records and the decoded-frame CRC pass. On
failure it retains the last verified frame, discards candidate state, and
reports the failing frame and check without logging pixel contents.

Output creation should be failure-atomic. The reference CLI writes an encoded
file through a same-directory temporary followed by atomic replacement. Before
staging, encode uses `lstat` and permits only an absent destination or an
existing regular file; it refuses symlinks, FIFOs, sockets, devices, and
directories without replacing the namespace entry. On POSIX it applies
`fchmod(0600)` to the open staging descriptor before writing, so a successful
replacement publishes a fresh private inode even under a restrictive or
permissive caller umask. Python `lstat` does not provide a universal no-reparse
guarantee for every Windows reparse tag, and Windows does not expose the same
POSIX mode contract, so those platform limitations apply to publication as
well as input. Decode immediately normalizes its POSIX staging directory to
mode 0700 and each staged PPM to mode 0600, then renames the directory only
after every output has been written. It rejects a destination path, including
a dangling symlink, that exists before publication. These portable publication
steps assume the output parent is not concurrently writable by an untrusted
principal; a future vessel needs an exclusive transaction or platform
no-replace primitive at that boundary.

Python staging names use path-neutral `.eidolon-write-*` and
`.eidolon-decode-*` prefixes rather than embedding the caller's basename.
Cleanup is best effort; tests observe removal on ordinary injected publication
and permission/write failures, but host cleanup failure, SIGKILL, power loss,
or a crash can still leave a mode-0600 temporary file or mode-0700 decode
directory containing mode-0600 private framebuffers. The caller therefore owns
retention cleanup in the output parent; this prototype does not claim
crash-recovery garbage collection.

The Ny CLI uses `write_bytes_atomic(path, content, replace)`, whose runtime
creates an exclusive no-follow same-directory temporary, completes and syncs
the write, and atomically publishes it. `encode` permits replacement of a
regular destination but rejects a destination symlink/reparse point.
`decode-frame` sets `replace=false`, so an existing file, directory, or dangling
symlink remains untouched. The Ny command validates the entire container before
selecting and publishing one requested frame. It intentionally does not offer
the Python CLI's multi-file decode command yet: publishing many PPMs requires an
atomic directory transaction that the Ny SDK does not currently expose.
Its path-neutral `.ny-write-*` staging file can likewise survive an uncatchable
termination and must be covered by the caller's retention policy.

## Required future evidence

- The observed five-route differential and atomic-publication cases remain in
  every future corpus: byte-for-byte deterministic encode, complete decode,
  no-follow input, no-replace output, and path-redacted failure through textual
  and binary NIL interpretation, LLVM JIT/AOT, and standalone textual NIL
  assembly and linking.
- Mutation and coverage-guided fuzzing under explicit memory and time budgets.
- Bounded streaming decode and publication with measured peak memory, decode
  time, compression ratio, mode histograms, and frame latency on named
  Anomalous traces.
- Capability and disclosure tests before any system capture rite exists.
- CPU feature gating and oracle equivalence before SIMD or optimized inline
  assembly can run.
