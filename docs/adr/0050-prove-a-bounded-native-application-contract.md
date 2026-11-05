# ADR 0050: Prove a bounded native application contract

- Status: Proposed
- Date: 2026-10-03
- Qualification: Bounded proof complete; all 33 matrix jobs and two restoration jobs passed on 291 unchanged frozen source files. This ADR remains Proposed and establishes no stable ABI.

## Context

The user selected the native application ABI baseline first, an ELF path next,
then comparison with the Linux Antimatter copy running on the host outside the
Anomalous VM. Windows and macOS comparisons follow later. This refines the
immediate work under accepted [ADR 0045](0045-select-native-antimatter-ny-and-hag-direction.md).
Antimatter is largely Electron today. This first baseline uses a bounded Rust
client that carries a small part of its editor semantics into independent
userspace; it is not a full Antimatter, Electron, CodeMirror, or Ny runtime port.
Ny remains the official language direction, with independent qualification
required before replacing verified components.

[ADR 0048](0048-prove-bounded-application-cpu-copy-and-fault-profile.md) supplies
the internal eager x87/SSE2, checked-copy, and selected-fault profile.
[ADR 0049](0049-qualify-a-bounded-freestanding-ny-object.md) qualifies one pinned
Ny object/startup composition. Neither defines application services. The
existing workspace content path places producer and compositor in one trusted
vessel. Its borrowed Rust frame and privately branded controls are not a wire
protocol or independent-task authority.

The next proof therefore separates a small application codec, an opaque copied
kernel channel, and userspace document/rendering policy. The contract is
experimental and exact to this version; it is not a stable general application
ABI. Reliquary packaging and executable framing remain separate from execution
and service semantics. ELF as a build input does not establish a Linux
personality, and the later ELF path must state its own supported personality.

## Decision

### Use canonical copied bytes without claimed authority

Add [`application.rs`](../../crates/anomalous-abi/src/application.rs) to the
experimental ABI crate. Encode every integer explicitly in little-endian
order; do not serialize Rust object layouts, references, or enums. Decoders
require exact packet lengths and reject unknown versions, nonzero reserved or
unused fields, unsupported values, and noncanonical text padding.

Neither packet contains task, principal, endpoint, window, focus, or grant IDs.
The trusted channel binding supplies exact producer/consumer task incarnations.
Packet sequences and document revisions provide correlation only; installed
grants, live task checks, and channel transition checks supply authority and
freshness. A successfully decoded packet is not an authorized operation.

The surface packet is exactly 3,168 bytes, below the existing 4,096-byte checked
copy bound: a 96-byte header and a complete 3,072-byte RGB8 frame. Header layout:

| Offset | Field | Version-1 requirement |
| ---: | --- | --- |
| 0 | Four magic bytes | `ANSF` |
| 4 | `u16 version` | 1 |
| 6 | `u16 header_size` | 96 |
| 8 | `u32 total_size` | 3,168 |
| 12 | `u32 flags` | 0 |
| 16 | `u64 request_sequence` | Nonzero; channel checks the exact sequence |
| 24 | `u64 document_revision` | Nonzero; model advances on text edits/undo |
| 32 | `u16 width` | 128 |
| 34 | `u16 height` | 8 |
| 36 | `u16 stride` | 384 bytes |
| 38 | `u16 pixel_format` | 1, RGB8 |
| 40 | `u8 text_length` | 0 through 31 |
| 41 | `u8 selection_anchor` | 0 through text length |
| 42 | `u8 selection_head` | 0 through text length |
| 43 | `u8 dirty` | 0 or 1 |
| 44 | `u8 theme` | 0 light, 1 dark |
| 45 | `u8 undo_depth` | 0 or 1 |
| 46 | 18 reserved bytes | All zero |
| 64 | 32-byte text area | Printable ASCII through declared length; remaining bytes zero |
| 96 | 3,072 pixel bytes | Complete opaque RGB8 frame |

Empty text is valid. Selection direction is preserved; the model normalizes
the range only when replacing it. Selection/theme changes do not advance the
document revision. The codec checks values, while the model and channel enforce
nonwrapping transitions. Pixel/semantic agreement is independently tested; the
codec does not infer that pixels actually depict the accompanying text.

The input packet is exactly 64 bytes:

| Offset | Field | Version-1 requirement |
| ---: | --- | --- |
| 0 | Four magic bytes | `ANIN` |
| 4 | `u16 version` | 1 |
| 6 | `u16 total_size` | 64 |
| 8 | `u64 sequence` | Nonzero |
| 16 | `u16 command` | 1 replace selection, 2 set selection, 3 set theme, 4 undo |
| 18 | `u8 text_length` | 0 through 31 for replacement; otherwise zero |
| 19 | `u8 theme` | 0 or 1 for set theme; otherwise zero |
| 20 | `u8 selection_anchor` | 0 through 31 for set selection; otherwise zero |
| 21 | `u8 selection_head` | 0 through 31 for set selection; otherwise zero |
| 22 | 10 reserved bytes | All zero |
| 32 | 32-byte text area | Replacement ASCII followed by zero padding; otherwise all zero |

Undo has no arguments. The model checks selection against its current text and
checks replacement capacity before mutation. This profile provides no claimed
human/machine source authority, raw keyboard input, IME, general Unicode,
clipboard, file selection, saving, or persistent storage.

### Keep channel transport opaque and task-bound

The kernel [`application_channel`](../../kernel/src/application_channel.rs)
owns one copied packet of 1 through 4,096 bytes. It binds an exact producer and
service, permits only one queued or delivered packet, and retains it until the
exact consumer acknowledges it. Prepare/copy/commit leaves no retained user
pointer. Failed admission or a returned copy error does not consume a sequence
or overwrite a queued packet. A private owner brand prevents prepared tokens
from being reused across distinct channel owners, even with repeated public
binding values.

Close occurs before either bound task retires. It discards and clears any queued
or delivered packet, invalidates pending transitions, and prevents further
delivery. Report logical buffer clearing without claiming physical erasure.
Channel delivery/acknowledgement does not attest rendering, presentation, or
semantic work completion. The kernel does not parse editor text or input
commands, arbitrate general focus, or adopt foreign syscall behavior.

### Prove one independent client and one service

The feature-private native runtime uses two independently owned tasks and two
oppositely directed channels: application surfaces to the service, and scripted
semantic input to the application. Each task has an inspected eight-page RX
extent, two private data pages, two user stack pages, and a separately owned
privilege stack. The pair retains 50 frames: 32 data frames and 18 table frames,
with ten bound leases. Eager CPU ownership and checked user copies follow the
existing application profile. Preparation, commit, dispatch, terminal teardown,
and restoration continue through retained owners.

The application model uses a single printable-ASCII line and one undo entry.
It ports the selected Antimatter insert-at-selection and appearance/selection
preservation behavior into allocation-free Rust. Source language and scope stay
visible in native evidence. The independently executing service validates the
copied surface and drives six scripted commands, observing seven complete
surfaces. Scripted semantic input does not establish physical keyboard routing,
focus policy, authenticated remote clients, or a deployed compositor.

The trace is grounded in Antimatter's production `insertAtCursor` and actual
CodeMirror history behavior. The host reference scripts
[`native-application-reference.ts`](../../scripts/native-application-reference.ts)
and its [runner](../../scripts/native-application-reference.cjs) preserve the
comparison outside the VM. The observed checkpoints are:

| Surface | Applied command | Text | Selection | Revision | Theme | Dirty / undo depth |
| ---: | --- | --- | --- | ---: | --- | --- |
| 1 | Initial state | `const before = 1;` | 0, 0 | 1 | Dark | false / 0 |
| 2 | Set selection | `const before = 1;` | 6, 12 | 1 | Dark | false / 0 |
| 3 | Replace selection with `after` | `const after = 1;` | 11, 11 | 2 | Dark | true / 1 |
| 4 | Set light theme | `const after = 1;` | 11, 11 | 2 | Light | true / 1 |
| 5 | Set dark theme | `const after = 1;` | 11, 11 | 2 | Dark | true / 1 |
| 6 | Set selection | `const after = 1;` | 8, 8 | 2 | Dark | true / 1 |
| 7 | Undo | `const before = 1;` | 6, 12 | 3 | Dark | false / 0 |

ASCII byte indices coincide with the reference's UTF-16 indices only for this
restricted text. The software renderer uses Anomalous's existing 5×7 bitmap
font and Antimatter's base colors; it does not reproduce browser typography.

The private x86-64 transport uses `int 0xf4`. Gate operations are startup (0),
submit (1), receive (2), acknowledge (3), cooperative yield (4), normal exit (5),
poll acknowledgement (6), stage a display row (7), and arm the exact fault
fixture (8). Replies distinguish success (0), unavailable/backpressure (1),
denied (2), bad copy (3), invalid operation/state (4), and closed (5). These
feature-private operation numbers are not additions to the established Rite
call prefix or a stable syscall contract.

Cooperative yields use the existing saved-frame scheduling transition. PIT
interrupts resume the same current task and are counted separately; reporting
timer ticks must not imply timer-driven task switches. Normal exit may select
the remaining peer, while last-current exit restores the bootstrap continuation.

### Defer verified presentation until both normal tasks retire

The userspace service scales the final 128×8 RGB8 surface by three and submits
24 complete 1,152-byte rows. Only that service has the fixture's staging grant.
The kernel retains these rows in a private fixed stage for the 384×24 rectangle
at `(32, 32)`. After both normal tasks retire and the bootstrap root is restored,
commit and read back that complete rectangle; verify pixels outside it remain
unchanged, then clear the stage. No framebuffer pointer enters userspace.

This intentionally deferred display proof is not an interactive compositor,
scanout completion, tear-free presentation, or a reusable application window
service. It separates copied application content from the prior same-vessel
path without promoting the test presentation mechanism into a general ABI.

In a second scenario, the producer queues one packet and reaches a separately
armed exact UD2 instruction. Revoke/clear both channels before the surviving
service resumes; require closed-channel behavior, one discarded packet, no
delivery, and no new presentation. The prior framebuffer must remain unchanged.
Retain task storage through both terminal states, restore kernel CPU/root/stack
state, detach scheduler contexts, and reclaim all owned storage.

## Validation and limits

The bounded proof passed canonical golden-byte tests, malformed
length/version/reserved/value cases, destination preservation on rejected
encoding, and model/renderer checks against the actual Antimatter reference.
Channel tests cover wrong/stale roles, private token ownership, backpressure,
copy failure, sequence exhaustion, acknowledgement and close in every phase.
Normal-exit-to-peer scheduler coverage also passed.

Native evidence must separately identify normal and fault scenarios, complete
surface/input transfer counts, cooperative switches versus timer ticks, rejected
operations, exact public-fixture surface hashes, verified display bounds, and
complete channel/CPU/memory restoration. Strict host validation rejects
missing, duplicated, reordered or mutated records. All 33 final matrix jobs
and both image-restoration jobs passed on 291 unchanged frozen files. Native
TCG and actual-host KVM passed both RAM sizes and separate captures with the
same proof image and identical public editor pixels. The final check includes
494 all-feature kernel tests, 460 native-feature kernel tests and 132 xtask
tests. Peak scope memory was 4.002 GiB under a 6 GiB hard cap, with no OOM event.
The [Native Application Log](../../notebook/Development/2026-10-03%20Native%20Application%20Log.md)
records final evidence, hashes, memory limits and the changed workspace image.

The harness requires separate 96-MiB and 320-MiB debug-exit boots with process
status 33, plus a capture boot with no debug-exit device. Capture waits for the
complete newline-terminated, strictly validated 21-record lifecycle before
requesting a screenshot. Verify the complete canonical P6 image length and each
pixel of the 384×24 public rectangle against the final reference frame scaled by
three. Host monitor quit must exit zero and is recorded separately, never as a
guest debug-exit result. Each process has one 30-second deadline and is reaped
on failure. Retain current-attempt public diagnostic logs and captures after
rejection rather than deleting the evidence.

The preceding four Antimatter host tests establish a behavioral reference only.
An original Linux application running on the host is not an Anomalous Linux
runtime, and this Rust client is not the full Electron editor. The later ELF
path must name its executable admission and execution/service personality.
General launch, native Reliquary framing, stable application ABI, dynamic
linking, TLS, threads, persistent workspace services, full editor parity, HAG
acceleration, and Windows/macOS execution remain outside this proof.

## Reversal

Remove the feature-private client, gate, runtime, channel composition, and
artifact/harness integration without changing ordinary boot or earlier accepted
proofs. Keep the experimental codec version distinct: do not silently widen its
geometry, text alphabet, commands, or field meanings. A wider native application
contract requires its own compatibility decision and evidence.
