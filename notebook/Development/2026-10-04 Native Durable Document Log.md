---
title: "Native Durable Document"
date: 2026-10-04
tags: [antimatter, native-abi, storage, durability, userspace, recovery]
status: initial_normal_path_qualified
---

# Native durable document — 2026-10-04

## Scope and qualification state

The initial normal-path implementation passed under proposed
[ADR 0063](../../docs/adr/0063-prove-native-durable-document-storage.md).
QEMU has now exercised native durable save, complete editor/service destruction
and reconstruction, then a new cold VM process recovering independently
generated text from the retained data disk. Invalid-media rejection also passed.
The required regression matrix and final emitted-code/source review are complete
as recorded below; broader native fault cases remain pending. Earlier RAM
document, native/ELF and retained-service replacement proofs remain separate
baselines.

This is a bounded Rust editor/service fixture supporting Antimatter's native
direction. The general [native filesystem](../../docs/architecture/filesystem.md)
remains unimplemented: this work does not provide directories, names, namespace
resolution, unlimited document storage, redundancy, snapshots or backups.
The five-sector and 31-byte limits belong only to the ADR 0060 checkpoint model,
not to native paths, filenames or future filesystem depth.

## Concrete mechanism and ownership

The temporary [device contract](../../docs/architecture/native-storage-device.md)
selects an explicitly added QEMU `isa-ide` controller on secondary legacy ports
`0x170..0x177` and `0x376`, IRQ 15, with only master/unit zero populated. It is
separate from the q35 AHCI boot controller. QEMU v10.2.1 primary source confirms
the controller properties and explicit `proof_ide.0` bus naming. The initial
live run has now qualified this provisioned normal path in QEMU TCG; it does not
qualify another controller, emulator or physical machine.

`kernel/src/arch/x86_64/native_storage_device.rs` implements only IDENTIFY,
one-sector PIO read/write and flush. One move-only device owner is claimed per
boot and retained by the coordinator across service destruction. No destructor,
task replacement, re-acquire or copied handle clears its ownership/poison state.
The controller is permanently poisoned for this VM after any attempted-I/O
failure; there is no in-VM reset or pretend fencing path.

The dedicated data extent is absolute sectors `8,13)`, bytes 4096 through 6655.
Every byte outside the extent, including guards, must retain its prior hash;
the boot image is checked independently. The same disposable raw data image
survives both cold launches. The runner must explicitly retain flushes, report
backend errors, prevent another writer, and avoid snapshots or development
scripts that recreate boot VDIs. The user's running VirtualBox VM is unrelated
and must remain untouched.

All device transactions run under the existing sole-BSP IF-clear contract.
Polling consumes one budget of 1,000,000 status samples across the whole
operation, not a PIT deadline or a wall-clock guarantee. IDENTIFY checks logical
sector size, capacity and flush support. Staged reads publish only complete
success; private sector buffers are cleared. Counters include every byte/word
port access as well as command/read/write/flush totals.

The architectural destination is a supervised userspace driver with explicit
device delegation and qualified fencing. This feature-private kernel transport
is the scoped ADR exception, not a permanent general driver or filesystem.

## Wire contracts and userspace policy

`crates/anomalous-document-protocol/src/durable_startup.rs` adds canonical
192-byte `ANDD` startup and 128-byte `ANDM` readiness records. Existing `ANDS`
RAM metadata keeps its meaning. Startup includes role/phase, 32 KiB arena,
four task-local routes, editor identity, persistent volume/object identity,
service device handle and optional challenge. It is copied at `0x801000` and
entered using RDI pointer/RSI length. Unused bytes are zero-checked. Only the
initial editor receives challenge content; the service never receives a seed
answer through its descriptor.

`kernel/src/arch/x86_64/native_durable_fixture.rs` reads the new QEMU `fw_cfg`
file `opt/anomalous-durable`. Canonical 48-byte `ANDI` input uses version one,
phase at offset eight, length at sixteen and printable text at seventeen.
Initial phase zero requires 1..31 bytes, while cold phase two requires no text.
The complete bounded directory scan rejects duplicates, missing entries,
malformed records and noncanonical padding without fallback. Fixture bytes
cannot authorize device access, choose a disk, or grant formatting authority.

`native_durable_profile.rs` retains exact current-task/frame/root validation and
adds native operations 12 (read), 13 (write) and 14 (flush). Read/write use RDI
service handle, RSI buffer, RDX 512 and RCX relative sector 0..4; flush requires
the three trailing arguments to be zero. Success returns status zero and the
sector/512 pair for transfers, or zero/zero for flush. Authority, copy, shape and
device errors use statuses 2, 3, 4 and 6 respectively, with cleared result fields.
Phase device handles are `0x7001 + phase * 0x100`, bound to the retained service
context; the editor cannot use them. Full write copying and full read-destination
preflight happen before port I/O.

The native durable client implements `sector_io` with these gates and uses the
existing safe `durable_store` for format, mount, record validation, checkpoint
checksums and durable save policy. Mount commits the new epoch before sending
`ANDM` at reply sequence one. The editor decodes and acknowledges readiness
before open; request N receives reply N+1. The kernel does not decode disk
records or invent the document epoch. A saved acknowledgement retains the
exact submitted snapshot and arrives only after required flushes.

## Implemented live sequence

Phase zero explicitly formats only a blank granted extent with empty initial
text, mounts, opens, inserts the host-generated challenge, saves, verifies the
saved snapshot, reads and closes. Both tasks exit; routes, CPU state, mapping
owners and all allocated frames/leases are retired. Phase one constructs a new
pair with zero-checked private storage, swapped arena placement and different
route/device handles. Its service mounts the retained disk and its editor
opens recovered contents without receiving expected text.

The runner then terminates that entire VM process and launches the same
inspected code for phase two. Only the separate data disk and evidence survive;
the recovery fixture has an empty challenge. The host retains expected text for
an independent byte/media/transcript/frame oracle. Rebuilding a guest with the
answer, restoring a paused VM, or supplying expected text to recovery would not
satisfy this proof.

The intended epoch/version/sequence progression is 1/2/3 after save, 2/2/4 after
same-boot service reconstruction and 3/2/5 after cold recovery. Every pair has
48 data frames, 18 table frames and twelve leases, then returns to its prior
allocator accounting. Both 32 KiB arenas, 8 KiB data regions and 8 KiB user
stacks are checked before startup initialization. Copied surface packets and
final rows reflect actual opened/saved text; no expected-text parser exists in
the kernel.

Native client denials cover editor device calls, unknown and prior-phase device
handles, null and region-crossing buffers, short sectors, out-of-extent and
overflowing sector numbers, and malformed flush arguments. Document probes
reject a different epoch, stale handle generation and conflicting save base
without changing store sequence. The initial target transcripts passed the
independently predicted counts and wire/pixel checks for these probes.

## Evidence available and outstanding

The first successful run is retained under
an external validation directory (not distributed).
Its [smoke receipt (`receipt.json`; not distributed)
records four successful debug-exit boots, three expected negative exits and a
separate host-quit capture. Local SHA-256 readback matches the retained manifest:

| Artifact | SHA-256 |
| --- | --- |
| Kernel ELF | `e9d68b432d58359fc3476f3c11cc7a6633d5d9a51ea6f09ca7326b5585a2eda6` |
| Native client ELF | `aca83765905c6f1923ceb4012a95703e8349c892da25e4daaddc29bd47b52df2` |
| Boot image | `2d32427cf9bf8dcfa71c742ce1d6167c4728ebab89f2be0f5773969efdcf2f58` |

QEMU q35/TCG with Nehalem completed provisioning/save plus same-boot full pair
reconstruction at 96 MiB and 320 MiB, followed by a new cold VM at each size.
Each size used its own 1 MiB disposable data disk. The challenge
`disk-50842576620dba57` was generated outside the compiled guest and omitted from
both recovery descriptors and the cold fixture. The host independently decoded
the exact saved bytes and version two, with epoch two after reconstruction and
epoch three after cold recovery. Provisioning transcripts contained 65 ordered
records; cold recovery contained 42, including the old 21-record native baseline.

Observed scalar evidence matched the source-derived oracle:

| Phase | Calls | Cooperative switches | Denials | Bad copies | Invalid requests | Arena copies |
| --- | --- | --- | --- | --- | --- | --- |
| 0: provision/save | 184 | 22 | 4 | 4 | 9 | 60 |
| 1: pair reconstruction | 143 | 16 | 5 | 4 | 9 | 52 |
| 2: cold recovery | 143 | 16 | 5 | 4 | 9 | 52 |

Each phase started/exited two tasks, checked 65,536 arena bytes and staged 24
rows, with resources restored. The first VM completed 33 device commands
(15 sector reads, nine writes and eight flushes, plus IDENTIFY); cold recovery
completed ten (five reads, two writes and two flushes, plus IDENTIFY). Neither
owner was poisoned. Polling-related port-read totals vary with completion timing
and are not treated as a fixed performance guarantee.

The independent media oracle compared every outside-extent byte, and all boot
copies and the inspected boot image remained unchanged. The separate capture
verified all 9,216 pixels in the 384-by-24 region at `(32,32)` and ended by host
quit; it was not counted as a fifth successful debug-exit boot. Three separate
negative boots rejected blank recovery, a corrupted header and provisioning on
nonblank media. They exited through `durable_unexpected_user_fault` as expected
by the negative oracle and left their entire data images byte-identical. They
did not silently format or fall back to a fixture.

The runner explicitly used writeback caching with flushes enabled. This first
receipt does not establish injected backend error handling: its command left
QEMU's read/write error actions at their defaults. The device contract records
that limitation separately from the successful normal-path result. The parent
subsequently added explicit `werror=report,rerror=report` to the data-drive
configuration. The final smoke rerun for that updated command passed, as recorded below;
the initial receipt is preserved with its original provenance.

The parent also reported successful serialized kernel host tests, including all
eleven transport mock-port tests, and successful native kernel Clippy. The tests
cover single ownership, local no-I/O rejection, geometry, exact transfers,
short reads/writes, completion failure, finite polling and permanent poison.
Separate mock counts check port access and failed reads preserve destinations.

ADR 0060's already-qualified portable crash model continues to cover torn writes,
partial flushes, uncertain outcomes and stale mount epochs under its modeled
assumptions. The current live client does not yet inject native service/device
faults before commit, between durability boundaries, or after commit before a
reply. It fails the run on uncertain device/store results instead of emitting a
false saved or semantic rejection. Lost-reply reconciliation and newer unsaved
edit cases from the full ADR remain pending and must not be marked complete by
a successful normal cold-recovery run.

## Final qualification and source binding

The final native smoke passed with challenge `disk-6c47efbfa4aa8d34`, independent
media/transcript/capture checks, the same four successful boots and three
negative boots, and one separate host-quit capture. The cold fixture contains no
answer. All eight actual QEMU argument vectors were independently observed and
retained in `final-smoke-qemu-processes.json`, including explicit writeback cache,
flush retention and read/write error reporting. `qemu-host-inventory.json` records
QEMU 10.2.1 and executable SHA-256
`0cd4112a8f0cb891eb7c10e8df38c9dfeec8c7389bb22db6aa425f0d6fe733dc`.

Final artifacts under `work/native-durable/final-native-artifacts/`:

| Artifact | SHA-256 |
| --- | --- |
| Kernel ELF | `b3d338452fbca83686fec4a02f4b8e58a2142e53a394a879a8d3ba175a057cc0` |
| Native client ELF | `aca83765905c6f1923ceb4012a95703e8349c892da25e4daaddc29bd47b52df2` |
| Boot image | `36af16308d877e6634522a2090b485684ed87c87c9318469f7aef5cb3d7fc5e7` |

The client bound is 5,408/8,192 stack bytes, including all panic paths; its RX
image is 23,603/32,768 bytes. Kernel bounds are 34,160/65,536 bootstrap bytes
(35,471 with one timer), 6,463/16,384 gate bytes, 1,591 fault bytes and 1,311
timer bytes. Seventy manual transfer sites, twelve kernel jump tables, both
client jump tables and 226 kernel local control-flow graphs were closed without
unresolved admitted indirects, recursion or growing stack loops. The review
confirmed complete preflight/private staging before I/O, volatile clearing,
poison retention, and revocation before releasing the runtime latch.

`client-review-final/` and `kernel-review-final/` hold those emitted reviews.
`final-kernel-binding/final-binding.json` binds them to the final exact binaries:
all executable bytes, initialized data, load geometry, entry, symbol table and
stack-frame metadata remain identical. Only four readonly bytes in three panic
source-line records changed; the image also updates eight checksum bytes. The
final client ELF is wholly identical to the reviewed client.

All 16 AGENTS-required commands passed in `required-matrix-results.json`, plus
native/ELF replacement and workspace regressions. The workspace verified 21
keyboard frames, 462 pointer/Appearance frames and 25 Graphics frames. Host
qualification included 234 xtask tests, 506 tests per kernel feature run, 32
document unit tests plus ten crash-model tests, and eleven transport mock tests
within the kernel runs.

An extra mixed-feature Clippy check exposed three unused arena helpers when
the older ELF application proof wins selection. Their compilation now follows
the selected native runtime predicate; both mixed-feature cases are permanently
in `make check`. After this conditional-only correction, final qualification
repeated native and all-feature kernel host tests, all-feature Clippy, native/ELF
replacement smoke tests and artifact binding. All passed. The ordinary matrix
was completed before this last conditional refinement; it is not described as
one uninterrupted run against a single unchanged source snapshot.

The release non-document source manifest contains 387 files, with SHA-256
`14f422d889c8733eeef810ecbc4a2690ad3ecf70e8b092177b4678c5ac0f8818`.
It was checked unchanged after qualification. Failed exploratory Clippy output,
earlier manifests and first-run artifacts remain retained separately.

The normal and workspace images were restored and remain byte-identical to the
launcher checkpoint (`4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762`
and `a9bd1589bdb910f92fea7cd1cf3d5e444e9c3719912e37c8986c505c707e5fa8`).
The native/ELF replacement images have new panic-location metadata/checksums;
their loaded instructions and initialized data remain identical to the earlier
qualified baselines. `relaunch-baseline-review-final/baseline-review.json` records
11 native and 15 ELF panic-line byte changes, plus eight checksum bytes per image.
Their final
reruns passed with full restoration and matching native/ELF bytes and pixels.

The isolated VirtualBox workspace probe passed without adopting the user's VM.
Its complete boot transcript matched the QEMU reference with explicit firmware
memory-map allowances; all 786,432 pixels matched, the private service/VM stayed
inside the guarded scope, and cleanup powered it off. The RGB SHA-256 is
`5c4bc135bded8d9be6839338cb3187622e7a0566929df25de943098c264e9041`.
This establishes workspace regression coverage, not native storage on VirtualBox.

All heavy jobs were serialized with two Cargo jobs, one test thread, MemoryHigh
4 GiB, MemoryMax 6 GiB and 512 MiB scope swap allowance. Across the retained jobs,
peak scope memory was 4,297,060,352 bytes (4.002 GiB); host available memory stayed
at or above 20,306,051,072 bytes (18.91 GiB). The large ordinary matrix recorded
5,107 soft-limit reclamation/throttle events, dominated by file cache, and no
hard-limit, OOM or OOM-kill events. The host OOM-kill counter remained zero.
The final native/VirtualBox group peaked at 1,381,621,760 bytes and recorded no
soft-limit events. `qualification-summary.json` retains the aggregate values.

Cold QEMU reconstruction fences its prior device process under host flush/cache
honesty assumptions. It is not physical host-power-loss testing. The format
persists content/version/epoch, not request receipts, anti-rollback freshness or
backup restoration safety. VirtualBox's application CPU-profile rejection is
unchanged; normal/workspace VirtualBox success does not qualify this native
storage profile. Full filesystem, interactive Antimatter and repeated editing
remain subsequent work.
