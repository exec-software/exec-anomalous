# ADR 0063: Prove native durable document storage

- Status: Proposed; bounded normal-path and native failure implementation qualified
- Date: 2026-10-04
- Related: ADR 0004, ADR 0013, ADR 0045, ADR 0048, ADR 0056, ADR 0058, ADR 0059, ADR 0060, ADR 0061, ADR 0062

## Context

The qualified native and ELF document proofs retain saved bytes only while a
RAM document service survives. Replacing its editor proves route and ownership
revocation, but does not prove recovery after destruction of the service.
[ADR 0060](0060-model-bounded-durable-document-recovery.md) qualifies a portable
five-sector checkpoint model with explicit save acknowledgement and uncertain
outcome semantics. At the start of this increment no live implementation of
its sector adapter existed. The current implementation has passed the initial
normal-path QEMU proof below and the separately qualified native failure
extension. General device fencing and physical power-loss qualification remain
open. Threshold's BIOS reads end before the runtime and provide no
application writes.

[ADR 0062](0062-design-native-object-filesystem.md) sets the broader filesystem
direction. This increment supplies a smaller live durability prerequisite; it
does not substitute the bounded checkpoint format for that filesystem design.
[ADR 0061](0061-select-re-extension-and-path-policy.md) continues to prohibit
fixed native path-length or depth ceilings. The capacities of this proof cannot
become filesystem, naming, path, or application-wide limits.

## Proposed decision

### A separate native proof and a narrow mechanism exception

Add an opt-in native durable-document profile with an editor and an independent
document service. Reuse the existing safe `durable` policy over `sector_io`,
task-bound copied channels, complete user-copy validation and exact saved
snapshot acknowledgement. Keep the qualified RAM clients, their semantics,
their native/ELF baseline behavior unchanged. Rebuilt artifact differences must
be reviewed and rebound to the qualified emitted code; diagnostic line metadata
is recorded separately from instruction or policy changes.

[ADR 0004](0004-capability-oriented-hybrid-kernel.md) places drivers and
filesystems in supervised userspace. This proposal permits only a temporary,
feature-scoped privileged transport mechanism because device delegation and
driver fencing are not yet qualified. It is not permission to add a permanent
general kernel driver, filesystem parser, port-access API or ambient disk access.
The destination is a supervised userspace driver behind the same explicit
device-ownership and failure contract; this proof must identify that boundary.

The implementation selects ATA PIO on an explicitly provisioned secondary ISA
IDE data channel in QEMU. The concrete
[device contract](../architecture/native-storage-device.md) records the exact
controller, registers, command subset, geometry checks and permanent poison
behavior, with primary-source references. Existing q35 or VirtualBox SATA/AHCI
boot configurations alone do not establish that legacy channel. The first live
proof has qualified the provisioned normal path; broader acceptance remains
subject to the requirements below.

The interfaces below are feature-private proof contracts, not stable native ABI
or general hardware specifications. AHCI, virtio, DMA, interrupt-driven I/O and
general PCI discovery are not implied alternatives within this bounded gate.

### Provisioned disk and sealed extent authority

Use a newly created, dedicated disposable data disk, separately identified and
retained by the proof coordinator. Keep the boot disk outside the grant and
verify its bytes remain unchanged. Do not scan or adopt host disks, existing VM
disks or the user's running development VM. No disk label, checksum, serial
string, relative sector number or copied startup field creates authority.

The trusted controller seals one exclusive extent grant to the exact admitted
document-service task incarnation, device ownership generation, extent and
permitted operations. The editor receives document routes only. There is one
writer and at most one synchronous device command in flight; no other alias or
service may access the granted extent or controller concurrently.

Expose only complete 512-byte reads, complete 512-byte writes and flush through
that grant. Relative-sector arithmetic must be checked before conversion to a
device address. Reject overflow and addresses outside the granted five-sector
model extent, including access to the boot disk. A flush has an explicit scope
covering the preceding completed writes in this exclusively owned adapter.

Every request must pass exact current task/generation, role, grant, operation,
device state, extent, length and complete memory-span checks before port I/O.
An integer that once named a grant must not authorize its successor task.
Copied descriptors remain information; retained kernel ownership is decisive.
Admission failure and stale, foreign or revoked grants perform no device I/O.

### Complete copies and bounded device execution

For a write, copy the entire sector into a private kernel buffer before any
device operation. For a read, preflight the complete destination's ownership,
permissions and mappings before issuing the command, then read into private
storage and copy out only a complete successful sector. A rejected request or
failed read must not publish partial destination bytes. Clear temporary bytes
before reuse. Never pass a user pointer to the device or expose a physical alias.

Retain the sole-BSP, IF-clear ownership contract across preflight, transfer and
reply. Do not enable interrupts inside an existing gate to obtain a deadline.
PIT ticks cannot advance while interrupts are excluded; bound polling by an
explicit finite work budget and report that distinction honestly. The budget
must cover each command phase without creating an unbounded busy loop.

Distinguish rejection before issue from uncertain completion after issue. If
completion cannot be established, poison the device grant and quarantine the
userspace store. A timeout, device error or short transfer must not become a
definitive negative save acknowledgement merely because the gate returned.
Further operations, replacement grants and remounts fail until prior commands
are known complete, aborted/reset or otherwise fenced.

Task destruction, clearing RAM and dropping a Rust borrow are not a hardware
fence. Keep controller quarantine attributable after service retirement. The
first profile may require termination of the exact isolated VM process and a
cold controller restart to clear uncertainty. In-VM retry after an uncertain
command requires separately qualified fencing; do not invent successful reset
semantics. Successful completed commands can support ordinary service remount.

### Userspace recovery and mount readiness

The kernel transports sectors as opaque bytes. Userspace alone validates the
header, volume/object identity, body/commit pairs, checksums, sequence, content
version and mount epoch using ADR 0060's policy. Recovery never formats invalid
media or substitutes a compiled fixture, startup seed or cached prior text.

Formatting requires an explicit provisioning action and a format-authorized
service route. Only `format_disposable` initializes the separately granted
extent, after verifying all five sectors are zero. Normal mount has no implicit
format fallback. A failed partial format requires explicit disposal/recreation
of that test medium; an ambiguous disk must not silently become an empty file.

Mount durably advances the epoch before admitting document operations. Persistent
volume/object identity, on-disk content version, mount/document incarnation,
task generation and open-handle generation remain distinct values. Task counters
restarting at cold boot must not revive an old document or device capability.

Add an explicit userspace mount-ready exchange over a task-bound opaque route
before the editor opens the document. It identifies this service's durability
contract and newly committed document epoch. Validate the complete correlated
exchange; neither an unsolicited record nor a field on disk authenticates it.
The kernel must not parse disk metadata to fabricate that epoch. Do not quietly
repurpose the existing RAM startup descriptor or change the meaning of its
document field; any additional startup/discovery encoding needs versioned review.

The durable service acknowledges a save only after the inactive body and matching
commit record have both passed the required flush boundaries. Retain the exact
document, handle, request, base version, editor revision and submitted bytes in
the acknowledgement check. A newer local edit remains dirty after an older save
completes. Existing RAM `saved` replies retain their RAM-only semantics.

Losing the reply after commit leaves an unknown outcome at the editor. Preserve
the pending snapshot and do not mark it clean, retry it as a new save, or infer
failure from a later conflict. Resolve through a safely mounted fresh owner and
an authenticated read of recovered bytes/version. Recovery reports committed
state; it must not claim a request failed merely because its receipt was lost.
ADR 0060 persists content, version and epoch, not a per-request receipt ledger.
Reopening reconciles current contents; it does not prove exact request history
or durable idempotency. Such receipts require a separate protocol and proof.

### Full service destruction and cold restart

Demonstrate successful open, edit, save and read through the real native service,
then close/revoke routes and destroy both editor and service owners. Withdraw
their contexts, mappings, private memory and CPU state, restore accounted
resources, and create a new pair. No retained RAM store or kernel text copy may
supply recovery. The new service must mount the same data extent and return the
actual saved bytes under a fresh epoch and fresh task/channel authority.

Then terminate the entire isolated VM process, retain only the dedicated data
disk and evidence, and launch a new VM process with the same inspected code.
Recover again through fresh owners. A saved-state restore, paused VM, retained
service, rebuilt payload containing the answer or host-injected recovery text
does not satisfy this gate.
Observed destruction and cold reconstruction fence the prior emulator device
instance, not every possible physical storage failure. Host cache/flush honesty
remains an assumption; cold VM restart is not a host power-loss experiment.

Use run-specific host-supplied challenge content delivered to the first editor
through a reviewed bounded fixture input. It is untrusted document data, not
authority or executable input. Generate it independently of the compiled guest
payload. The recovery launch receives identity/provisioning information but no
expected text. The host retains expected bytes solely for its independent oracle.
The 31-byte printable-text capacity remains an explicit test-model boundary.

Retain explicit flush/cache settings and the actual controller identity. The
VirtualBox requirements documented in ADR 0060 include the correct data LUN's
flush handling; QEMU must not suppress flushes or use unsafe/snapshot storage.
Do not use `scripts/run-virtualbox.sh` to retain the data disk: that development
workflow deliberately recreates its boot VDI. Extend only an isolated proof
workflow with explicit ownership and retention of its separate data medium.

## Current implementation contract

`native_durable_test` selects a separate native client and runtime, retaining
the previous 21-record native application baseline. It does not change the RAM
document/relaunch clients or create an ELF durable profile. The current live
increment implements normal durable save, complete pair teardown/remount and a
cold recovery launch, plus authority/copy/shape denials. Native service/device
fault cuts and the lost committed-reply scenario remain unimplemented; the
broader acceptance requirements below remain open.

### Device transport and task-bound operations

`kernel/src/arch/x86_64/native_storage_device.rs` owns the explicit QEMU
`isa-ide` secondary channel at `0x170..0x177`/`0x376`, IRQ 15, master/unit zero.
The boot disk stays on its existing controller. The granted data extent is
absolute LBAs 8 through 12, or `[8,13)`, with all outside bytes and the boot disk
covered by independent before/after hashes. The host retains that exact data
image across the two VM processes. No disposable boot-image conversion routine
may recreate or replace it during recovery.

The coordinator claims the device once without port I/O, then performs trusted
read-only IDENTIFY qualification before any application access. The same
move-only owner remains on the suspended coordinator stack across both service
lifetimes in the first VM. IDENTIFY requires 512-byte logical sectors, LBA28
capacity covering the extent and supported/enabled flush. One finite budget of
1,000,000 alternate-status samples covers a complete operation. The mechanism
has no reset or release API; poisoning cannot be cleared by replacing a service.

The native `int 0xf4` profile adds only these device operations:

| Operation in RAX | RDI | RSI | RDX | RCX |
| --- | --- | --- | --- | --- |
| 12, sector read | Sealed service handle | Complete writable destination | 512 | Relative sector 0..4 |
| 13, sector write | Sealed service handle | Complete readable source | 512 | Relative sector 0..4 |
| 14, flush | Sealed service handle | 0 | 0 | 0 |

Successful reads/writes return RAX 0, RCX equal to the requested relative sector
and RDX 512. Flush returns zero in all three result registers. Errors return
RAX 2 for authority, 3 for copy admission, 4 for shape/extent, or 6 for a failed
device transaction; error RCX/RDX are zero. Only the selected exact retained
service context can use its phase's device handle (`0x7001 + phase * 0x100`).
The editor has no grant. The existing saved-frame/root/task checks precede the
storage handler; handle integers alone do not confer ownership.

The runtime preflights the complete read destination, copies a complete write
source, and rejects malformed requests before calling the transport. Sector
data stays opaque to the kernel. Every port read/write and issued/completed
command is counted, including status and qualification accesses. The client
maps any nonzero device result to `io_error::failed`; an uncertain store result
fails the current proof rather than fabricating a semantic rejection or saved
acknowledgement.

### Copied startup, readiness and fixture input

`anomalous_document_protocol::durable_startup` defines new canonical formats;
the existing RAM `ANDS` remains unchanged. The 192-byte `ANDD` version-one
descriptor is copied at `0x801000`, entered through RDI/RSI pointer/length, and
contains role, phase, private arena extent, four task-local channel handles,
editor task identity, persistent volume/object identity, service device handle
and optional challenge bytes. Reserved bytes must be zero. Only the phase-zero
editor receives a nonempty challenge; every service and recovery descriptor
contains no expected document text. Role zero has no device handle.

The service mounts through the existing userspace `durable_store`, committing
the new mount epoch before sending canonical 128-byte `ANDM` version-one
readiness. It carries document epoch, content version and checkpoint sequence,
with `sequence = epoch + version` and zero reserved bytes. It is reply-channel
sequence one; the editor validates and acknowledges it before open. Document
request sequence N receives reply sequence N+1. The kernel transports these
records without interpreting disk identity, epoch or document policy.

`native_durable_fixture.rs` reads exactly one
`opt/anomalous-durable` QEMU `fw_cfg` entry. Its 48-byte `ANDI` version-one input
contains phase, challenge length and up to 31 printable ASCII bytes. Phase zero
requires nonempty text; phase two requires an empty payload. The directory scan
is bounded and rejects missing, duplicate or malformed entries without a seed
fallback. This fixture supplies untrusted test data, not device or formatting
authority, and its small capacity is unrelated to native filesystem path limits.

### Pair destruction and independent recovery

The first VM executes phase zero: explicit blank-only formatting with empty
initial content, mount, open, challenge insertion, exact durable save, read and
close. It then destroys both tasks, withdraws contexts, revokes routes, clears
CPU/memory owners and restores resource accounting before phase one constructs
an entirely new editor/service pair. Phase one only mounts and opens the same
disk; it receives no challenge. Phase two repeats recovery in a new cold VM
process with the same inspected guest code and retained data image.

Each pair uses two separate four-region owners with 32 KiB private arenas,
48 data frames, 18 table frames and twelve leases. Private data, user stacks and
arenas are checked zero before startup publication. The same-boot remount swaps
arena placement, uses new channel/device-handle values and must restore all
pair resources again. Document epochs come from successful userspace mount,
not reset task counters or kernel parsing.

Expected successful checkpoint states are epoch/version/sequence 1/2/3 after
the first saved challenge, 2/2/4 after same-boot remount and 3/2/5 after cold
remount. The host must construct its own record/media oracle and compare actual
challenge bytes, metadata, packet/surface traces and final rendered state. It
must not import the guest's recovery implementation to supply expected results
or provide the challenge to either recovery descriptor. Initial live evidence
is recorded below, followed by the final source/artifact and emitted-code binding.

## Initial measured normal-path proof

The first `native-durable-smoke` passed on QEMU q35/TCG with the Nehalem CPU
profile at both 96 MiB and 320 MiB. Each size completed a provisioning/save VM
containing full pair teardown/remount, then a new cold recovery VM against the
same 1 MiB disposable data disk. The four successful boots used the same boot
image, with SHA-256
`2d32427cf9bf8dcfa71c742ce1d6167c4728ebab89f2be0f5773969efdcf2f58`.

The retained challenge was `disk-50842576620dba57`; the cold fixture supplied no
expected text. The host's independent media decoder verified the saved bytes,
content version two and mount epochs two/three after same-boot/cold recovery.
Every outside-extent byte and the boot image remained unchanged. A separate
host-quit capture verified all 9,216 pixels in the 384-by-24 application region.

Three separate invalid-media boots also failed closed: mounting a blank extent,
mounting a corrupt header, and requesting provisioning on nonblank media. Each
produced the expected failure exit and left the entire data image unchanged.
These are rejection proofs, not successful recovery or injected commit failures.

The [development log](../../notebook/Development/2026-10-04%20Native%20Durable%20Document%20Log.md)
binds the initial kernel/client/image and retained receipt. This establishes the
normal path and media refusals only. The final rerun with explicit
`werror=report,rerror=report` passed the same eight VM processes with the new
challenge `disk-6c47efbfa4aa8d34`. Its image SHA-256 is
`36af16308d877e6634522a2090b485684ed87c87c9318469f7aef5cb3d7fc5e7`.
All 16 required checks passed, along with native/ELF replacement and workspace
regressions. Final emitted-code review is bound to that exact kernel/client;
source selection fixes received focused repeats. An isolated VirtualBox
workspace matched all 786,432 QEMU pixels; this does not qualify native storage
on VirtualBox. The development log records final hashes, stack bounds, command
provenance and memory events.

The separate extension below now qualifies completed-I/O service cuts, lost
committed replies, newer unsaved edits and injected completion-observation loss.
Portable torn-write tests remain distinct modeled evidence. ADR status remains
Proposed: this bounded emulator proof does not complete general driver fencing,
physical power-loss qualification or the larger filesystem architecture.

## Native failure extension — bounded qualification complete

The separate `native_durable_failure_test` profile passed seven cases at both
96 MiB and 320 MiB, with cold reconstruction and independent media/transcript
verification, plus fourteen public-frame captures. All 16 required checks, the
normal storage proof, native/ELF replacement and workspace regressions passed.
Exact artifact, stack, source and memory evidence is retained in the
[failure log](../../notebook/Development/2026-10-04%20Native%20Durable%20Failure%20Log.md).
The successful normal profile and its evidence remain separate.
This extension tests service faults at specified completed-I/O boundaries,
delayed save acknowledgement with a newer edit, and deterministic loss of device
completion observation. It does not add abrupt VM-process killing or claim
physical power-loss behavior, spontaneous ATA ERR delivery or a measured timeout.

Use fresh disposable blank media for each case. Explicit formatting establishes
A as empty content at version one. B is the independently generated challenge
supplied only to the initial editor; C is a newer local edit made before the
target save completes in every case. The host independently predicts both A
and B, rather than treating empty content as a recovery fallback. The same fixed five-sector extent and outside
guards apply. A cold reconstruction receives identity/case information but no
expected document bytes.

| Case | Boundary or behavior | Allowed recovered content |
| --- | --- | --- |
| 1 | Service fault before executing the delivered target save | A |
| 2 | Service fault after the target body write succeeds | A |
| 3 | Service fault after the body flush succeeds | A |
| 4 | Service fault after the commit write succeeds, before final flush | A or B |
| 5 | Service fault after final flush succeeds, before saved reply | B |
| 6 | Saved B reply is delayed while the editor makes C; exact B acknowledgement leaves C dirty | B |
| 7 | Final target FLUSH is issued but completion observation is deliberately lost | A or B |

The dedicated client arms observation around the target `durable_store::execute`
save, keeping format/mount traffic outside the case. Its `sector_io` wrapper
places cases 2–5 after the corresponding successful operation; it does not
modify the portable checkpoint policy. Exact inspected service fault markers,
current-task ownership, channel state and device counters validate the selected
boundary. Closing the failed service's routes must let the surviving editor
mark its exact pending snapshot unknown without accepting a saved reply. Both
owners must then retire with their original accounting and authority revoked.
Recovered contents reconcile state, not a persisted request receipt ledger.

The versioned failure startup is `ANDD` version two, still exactly 192 bytes,
with case `u64` at byte offset 144 and zero reserved tail from 152 onward. Existing
fields and the challenge restrictions remain; version-one normal descriptors
do not silently acquire a case field. The failure `fw_cfg` input is `ANDI`
version two, exactly 56 bytes, with case `u64` at offset 48. It retains initial
phase zero with challenge and cold phase two with no challenge. Cases are 1..7;
normal profiles reject this extended fixture. `ANDM` readiness continues to
report the epoch/version actually recovered and committed by userspace mount.

For case 7, only the trusted feature controller may arm
`device::arm_flush_observation_loss()` after the target save has been delivered
to the exact service. Arming performs no port I/O and requires healthy,
identified, IF-clear state with no prior pending arm. The first subsequent
FLUSH completes normally. The second is actually issued, then its remaining
observation budget becomes zero before any completion-status sample. Existing
`finish` → `poll_exhausted` → poison handling returns failure; no successful
completion or saved acknowledgement is fabricated. The command may still have
completed, so the allowed outcome is explicitly unknown.

The store must report `outcome_unknown`, retain quarantine and reject subsequent
policy execution. Valid service read/write/flush probes must then fail with
unchanged port counters and destination bytes. `device::grant_available()` is
a local no-I/O preflight that rejects unidentified, IF-enabled, armed or poisoned
state. After complete pair retirement, the same retained owner must remain
poisoned and refuse successor grant/remount admission without device access.
Only terminating that isolated VM and cold reconstruction clears its lifetime.
There is no in-VM unpoison or reset path. All injection fields, methods and
branches are compiled only for the failure feature.

Acceptance of this extension requires its own exact artifacts, independently
predicted transcripts/media/frames, fault-marker and stack review, and serialized
regressions. The four added mock-port cases check arming/regrant rejection,
actual second-FLUSH issue with no later sampling, poison after the service borrow
ends, and preservation of an earlier device failure. These mock cases and the
actual target teardown/cold-recovery proof passed independently. All forty-two
QEMU argument vectors were observed with explicit backend error reporting.
The two uncertain cases recovered complete B in this run; their contract still
permits A or B and does not promote a missing completion observation to success.

## Acceptance evidence

Before accepting the implementation, require all of the following:

1. Host tests of grant admission, revocation, integer overflow, exact sector
   lengths and complete copies. Wrong identity, wrong role, stale generation,
   out-of-extent requests and malformed read/write pointers leave device-I/O
   counters and destination bytes unchanged. Repeat meaningful denials in ring 3.
2. Controller qualification before writes: reject unsupported or absent devices,
   wrong geometry and unsupported flush behavior. Record issued/completed
   operation counts, finite polling limits and poison/fencing transitions.
   Test rejection after poisoning without pretending to simulate host OOM.
3. Independent host verification of exact saved/recovered bytes and versions,
   complete transcript ordering, relevant rendered state, resource restoration
   and stale-authority rejection. Expected recovery must not be derived by
   calling the guest store or its record encoder. Retain before/after media.
4. Hash every byte outside the granted extent, including test-disk guard regions,
   and compare the untouched boot image. Verify the host backend has no hidden
   second writer or disk replacement between the two cold launches.
5. Fault cases before save execution, after body write/flush, after commit write,
   and after final flush but before reply. Require the appropriate old snapshot,
   allowed old-or-new uncertainty, or mandatory new snapshot at each boundary.
   Never accept mixed content or a false clean editor. Preserve the existing
   independent torn-write and partial-flush model tests and their stated limits.
6. Full pair destruction/remount and cold-VM recovery of the challenge content,
   with new mount epochs, rejected old handles/routes and no seed fallback.
   Include unsaved newer edits and a lost committed-save reply as distinct cases.
7. Exact artifact/source binding, emitted stack review, complete checked-copy
   and privilege-boundary review, and a frozen source manifest for qualification.
   Run all repository-required checks, relevant legacy application/RAM/ELF
   regressions and the actual new native proof under serialized memory limits.
   Retain failures, peak memory and separate soft-limit, hard-limit and OOM events.

QEMU is the first native execution target for this gate. Existing application
profiles reject VirtualBox's masked AMD `XSaveErPtr` capability; do not weaken
that CPU-state guard to obtain a storage result. VirtualBox native application
qualification requires its own resolved CPU-profile evidence. Normal/workspace
VirtualBox success does not qualify the application profile or durable storage.
Do not interrupt or repurpose the user's interactive VM for these tests.

## Dependencies and limits

A real interactive Antimatter frontend, general application launch and repeated
editing remain separate integration work. The current fixture has a bounded
single-undo model; persistence must not be used to imply an unbounded editing
history, arbitrary document size or an Electron/Ny runtime port.

This gate proves one explicitly owned durable document under ADR 0060's stated
device assumptions. It does not stabilize a filesystem format or namespace,
implement multiple files/directories, prove independent media redundancy, provide
snapshots/backups, or establish physical host-power-loss resilience. Alternating
checkpoint pairs on one disk are neither independent replicas nor a backup.
Backup restoration and media rollback are excluded: replaying an old disk can
reuse its epoch, so the mount counter alone provides no anti-rollback guarantee.
Broader storage guarantees require their own implementation and recovery evidence.
