# ADR 0051: Prove userspace ELF loading against native

- Status: Proposed
- Date: 2026-10-03
- Qualification: Bounded proof complete on 2026-10-04; all 35 matrix jobs and two restoration jobs passed on 302 unchanged frozen source files. This ADR remains Proposed and establishes no stable ABI.

## Context

After the bounded native contract in [ADR 0050](0050-prove-a-bounded-native-application-contract.md),
the user selected ELF next and comparison against the same editor baseline.
Executable container parsing and execution personality are separate boundaries.
This slice qualifies a narrow ELF loading path while keeping native startup,
INT f4 services, CPU ownership, and the userspace editor/service semantics.
At this checkpoint a real Linux startup/SYSCALL adapter was the next gate;
[ADR 0052](0052-prove-linux-startup-and-record-device-syscalls.md) subsequently
qualified that bounded comparison. This ELF proof itself establishes neither
Electron compatibility nor a second execution personality.

## Decision

Parse ELF in an independently executing, unprivileged loader vessel. A shared
allocation-free, safe Rust adapter accepts a canonical sectionless ELF64,
little-endian ET_EXEC/EM_X86_64 image with exactly two PT_LOAD headers. It requires
an RX segment at 0x400000 (at most 32 KiB), an RW segment at 0x800000 (at most
8 KiB), and entry exactly 0x400000. RX file size equals memory size; RW file size
is at least the 128-byte startup block and no greater than memory size. File
spans are aligned, increasing and disjoint, all arithmetic is checked, gaps
are zero, and the file ends at the last load byte. The complete deployment
artifact is at most 64 KiB. No interpreter, dynamic linking, relocations, TLS,
extra segments, section tables, alternate CPU profile, or address randomization
is admitted. Physical-address fields convey no allocation request or authority.

The host inspects the full linked client and creates a compact deployment ELF
preserving both segments' exact file bytes and virtual geometry. The full ELF,
disassembly, compact ELF and manifest are retained. The guest loader parses
the compact file itself; it does not receive a pre-parsed plan as input.

The loader submits a copied 96-byte inert plan through experimental gate
operation 9. The little-endian plan contains ANEL magic, version 1, length 96,
entry, two segment extents (file offset/size and virtual address/memory size),
image length, and a zero reserved word. The kernel decodes plan geometry and
requires equality with the independent host-inspected immutable manifest; it
does not interpret ELF headers. This comparison binds the offered extents to
the instruction-inspected payload, not merely any bytes in kernel rodata.
Controller-owned task identity and role authorize the offer. The plan contains
no task, principal, channel, grant, physical-frame, or page-root identifiers.

The loader has no application channel grants and may only start, offer a plan,
and exit. Only role zero may offer once. Wrong role, failed copy, short/corrupt
plan, changed geometry and duplicate offer are rejected without creating an
application task. Both loader tasks retire, their contexts are withdrawn, and
all pages/CPU state/alias ownership is restored before application allocation.

The controller then runs the same inspected client bytes through two routes:
native flat spans and the accepted ELF plan's spans. Both use the same fixed
owner/permission profile and two task roles (editor and service), with normal
and queued-packet fault scenarios. Initialized writable bytes are copied into
inactive owned mappings and read back before commit. BSS, page slack and private
stacks are verified zero. A feature-only nonzero data canary at 0x800080 is read
with a volatile load by each client task. The default native fixture retains
its previous all-zero writable-data contract.

Compare all application counters except nondeterministic timer ticks, seven
surface packet hashes, final framebuffer bytes by direct comparison and host pixel oracle,
outside-patch preservation, queued-packet revocation and complete owner cleanup.
Each pair uses the existing 50-frame peak; pairs are sequential. There are two
loader tasks and eight application tasks, with two separate presentations of
the same final frame. The existing native transcript describes the ELF-derived
route, followed by explicit loader/comparison receipts.

The ELF proof reserves its large suspended runtime in feature-private supervisor
BSS. An exclusive construction/reuse latch covers field-by-field initialization,
CPU preparation at the final stable address, publication and teardown. A failure
retains storage and ownership. Only complete context/resource/CPU cleanup and
withdrawn handler pointers permit reuse; no fallible cleanup is delegated to
Drop. Transient mapping preparation remains out of line. The 64 KiB bootstrap
stack and its 16 KiB minimum headroom check are unchanged. Default native runtime
storage remains on its previous stack path.


## Limits and consequences

This is an exact experimental profile, not a stable executable or loader API.
The loader and application are trusted inspected fixtures; this is not arbitrary
file admission. The kernel's inert-plan check still requires controller policy,
immutable source ownership, exact mapping geometry and cleanup. Parsing success
alone proves none of those properties. Rust remains the implementation language
for this proof while Ny's independent adoption gates remain unchanged.

The execution-ABI follow-up selected at this checkpoint was explicit Linux
startup, SYSCALL entry and a bounded syscall personality over shared service
operations. ADR 0052 now records its qualification; production foreign policy
still needs a supervised userspace veil under ADR 0008.
ELF container admission must never imply Linux authority or compatibility.
Windows and macOS require their own container and personality qualification.
Displacer is a separately recorded future desktop integration direction.

## Validation

All 35 matrix jobs and two image-restoration jobs passed on 302 unchanged frozen
source files. Evidence includes nine parser/wire test groups, 142 host harness
tests, artifact inspection, TCG and actual KVM boots at 96 and 320 MiB, separate
public-frame captures, normal-image exclusion and the full repository checks.
Both backends used identical ELF proof images and kernels; their editor-region
pixel bytes match exactly. The final matrix peaked at 2,045,624,320 bytes
(1.905 GiB), with no soft/hard limit or OOM events and at least 21,207,310,336
bytes (19.751 GiB) of host available memory. Normal and workspace images were
restored with hashes unchanged from the preceding native checkpoint. See the
[ELF application log](../../notebook/Development/2026-10-03%20ELF%20Application%20Log.md).
