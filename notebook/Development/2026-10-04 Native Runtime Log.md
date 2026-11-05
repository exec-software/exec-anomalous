# Native startup and private arena proof — 2026-10-04

## Scope

Continue native ABI work toward Antimatter with supplied initial document state
and private userspace working memory. ADR 0055 records the experimental contract.
The existing native, ELF/Linux and shared veil artifacts remain reference gates.
This is still a bounded Rust editor client, not an Electron port.

The same new native binary runs with two documents, swapped private arena bases
and two handle families. It allocates editor state and render/transport buffers
from a precommitted 32 KiB arena. The sequence runs document one, a guard-write
fault, an NX fetch fault, then document two. Kernel and guest inspect all arena
bytes before use; teardown must restore all 66 frames and twelve leases after
every pair. Kernel bindings remain authoritative independently of startup bytes.

## Early findings

The first host run exposed the existing three-mapping circle limit. Adding a
four-region prepared launch alone could not attach it. The fix gives only the
explicit arena circle-table constructor a four-mapping limit; ordinary tables
retain three. New tests cover both limits and failure atomicity.

The first xtask host run passed all new checks but hit an existing desktop
lock-reacquisition test. This is consistent with concurrent test processes
briefly inheriting a flock descriptor before exec; that cause was not directly
measured. Qualification therefore serializes test threads as well as compiler/QEMU jobs;
the original failed receipt remains available. No production lock semantics
were changed.

## Native VM and artifact evidence

The native profile passed TCG/Intel and KVM/AMD at both 96 MiB and 320 MiB, plus
an independent host-quit capture on each backend. Each transcript has 41 exact
records. Both complete frame captures match byte for byte, and the final public
patch renders `const before = 2;` with the restored selection. Normal scenarios
record 114 calls, fourteen cooperative switches, seven surfaces, six inputs,
fifty arena copies and 24 presentation rows. Fault scenarios record fourteen
calls, one discarded surface, one contained fault, one surviving service exit
and no published rows. Alternate-layout handle rejections leave channel state
available for subsequent progress. Every pair returns all resources.

The client ELF is `2ec88551353b860e58a8baca6fec1dd11b2ecd4374ea203a50a931e29ca26c5e`.
Its RX image is 10,739 bytes and initialized zero RW storage is 4,224 bytes.
Independent inspection bounds its stack at 1,344 of 8,192 bytes, including fatal
assertion paths. All four compiler jump tables stay within their functions; the
only other indirect branch is the deliberately armed NX jump. Startup preserves
RDI/RSI, and the checked call shim preserves the SysV callee-saved register it uses.

The native kernel is `eba241fbf31370c442a7a044e4b1ed48c481c7156510ffbdd2abb7284242d842`.
The conservative bootstrap bound is 24,376 of 65,536 bytes. Including the
normalized frame, alignment and return slot, gate/fault/timer bounds are
5,911/1,591/1,311 bytes of their 16,384-byte transition stacks. These estimates
are bound to the retained exact binaries and inspected indirect call targets;
they are not a general guarantee for arbitrary application or kernel code.

The artifact gate caught a linker location-counter error and a POSIX `nm`
format mismatch during early builds. It also rejected compiler-emitted readonly
string metadata. The corrected inspector admits exact `ALLOC|MERGE|STRINGS`
metadata only for this new profile; preceding native/ELF/Linux acceptance is
unchanged. Exact startup, transport and fault-arm bytes are checked in addition
to complete section/load permissions and CPU instruction-profile inspection.
All early failed receipts remain retained. The first full check later stopped
on an unnecessary lifetime in the new host inspector; it was elided, the new
configurations passed lint, and qualification restarted on a fresh frozen set.
No kernel or client source changed for that lint repair.

VirtualBox is installed, but `/dev/vboxdrv` is not readable or writable by this
user. No VirtualBox boot is claimed. Threshold itself was unchanged; TCG and
actual KVM provide this slice's boot evidence.

## Validation

All **43 qualification checks** passed on the unchanged frozen set of **340
source/configuration files**, followed by both development image restorations.
This includes every repository-required check, the earlier ABI proofs, desktop
and workspace regressions, the real host Antimatter reference, TCG/Intel and
actual KVM/AMD runs, and independent captures. Host tests pass 491 kernel tests
for the native profile, 531 with all features, 172 tooling tests, six runtime
codec/allocator tests and ten editor-contract tests.

The final native kernel and client match the independently reviewed binaries.
All twelve preceding embedded blobs remain unchanged. The five earlier ABI
profiles also retain equal TCG/KVM whole captures. Normal and workspace images
are restored; the normal image is byte-identical to the preceding checkpoint.

No host or cgroup OOM occurred across any attempt. The final matrix peaked at
2,974,629,888 bytes (2.770 GiB), with a minimum of 15,638,114,304 bytes
(14.564 GiB) host available memory. It recorded zero soft-limit,
hard-limit and OOM events. Across all attempts the peak was 4,237,074,432 bytes
(3.946 GiB); failed runs were assertion, build or lint failures,
never memory-pressure aborts.

| Artifact | SHA-256 |
| --- | --- |
| anomalous.img | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| workspace-dev.img | `bd4a51a6e7a8ab9b6d9e4c1ce4df2112d2c21c85e70eeaa66b26b5e8419266ca` |
| native-runtime-test.img | `15cacf474519d9baadcdf2faa21660f1bb0121c600c68ac5b919952f471c6306` |

Evidence is retained under
an external validation directory (not distributed).
Compiler/QEMU jobs run serially with two compiler jobs, one test thread,
`MemoryHigh=4G`, `MemoryMax=6G`, and `MemorySwapMax=512M`. A two-second monitor
stops the job on a new OOM event or host available memory below 4 GiB.

## Git checkpoints

The user requested frequent commits and pushes. The exact previously qualified
recovery/native/ELF/Linux/shared-veil snapshot was committed and pushed as
`16bd390` independently of this slice. This native-runtime increment is a
separate tested checkpoint. Repository guidance now records the preference
for small, tested commits and regular pushes. Generated images, binaries and
local qualification evidence remain outside Git.

## Next gate

Use the supplied clean document baseline to add bounded native document
open/save service semantics, with authenticated document handles and exact saved
snapshot acknowledgements. ELF comparison should consume that same native
contract after the native service is qualified. Threads/TLS, asynchronous input,
richer presentation and production Electron facilities remain separate gates.
