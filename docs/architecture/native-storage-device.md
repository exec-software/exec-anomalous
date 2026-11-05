# Native proof storage device

This is the concrete, temporary mechanism proposed by
[ADR 0063](../adr/0063-prove-native-durable-document-storage.md). It is private to
the durable-document proof. It does not establish a general block ABI, a
permanent kernel driver, or the native filesystem format. Qualification results
belong to the proof's retained evidence. The initial normal-path QEMU save,
pair-destruction/remount and cold-recovery run has passed, with the limited scope
recorded below. The separate failure extension also passed completed-I/O
service cuts, lost replies, newer unsaved edits and injected loss of completion
observation, with exact emitted-code binding and final regressions.

## Provisioned topology

The first target is an isolated QEMU x86-64 VM, with a dedicated secondary ISA
IDE channel. The existing q35 AHCI boot disk remains separate. QEMU 10.2.1's
[q35 implementation](https://github.com/qemu/qemu/blob/v10.2.1/hw/i386/pc_q35.c)
creates an ISA bus and an ICH9 AHCI controller; q35 alone does not provision this
proof's legacy channel.

The runner explicitly supplies the following additional device configuration:

```text
-device isa-ide,id=proof_ide,iobase=0x170,iobase2=0x376,irq=15
-drive if=none,id=proof_data,file=DATA_IMAGE,format=raw,cache=writeback,cache.no-flush=off,werror=report,rerror=report
-device ide-hd,drive=proof_data,bus=proof_ide.0,unit=0,logical_block_size=512,physical_block_size=512
```

`DATA_IMAGE` denotes the coordinator's newly created disposable regular file;
the runner must escape its actual argument correctly. No host block device,
existing VM disk, snapshot overlay, second writer or hotplug is admitted. The
runner retains disk size, media, transcripts and integrity receipts. The controller
and drive arguments above define the configured topology; executable/version and
observed process arguments are separate host qualification evidence. The runner
does not itself query the complete live device tree. Device admission still
requires real IDENTIFY success in the guest. A topology or QEMU-version change
needs requalification.

The explicit register-base/IRQ properties are supported by QEMU's
[ISA IDE device](https://github.com/qemu/qemu/blob/v10.2.1/hw/ide/isa.c). Its
[IDE bus](https://github.com/qemu/qemu/blob/v10.2.1/hw/ide/ide-bus.c) has two units;
only master/unit zero is populated. The `proof_ide.0` bus name follows QEMU's
[parent-ID bus naming](https://github.com/qemu/qemu/blob/v10.2.1/hw/core/bus.c).
The entire channel is exclusively reserved even though only one disk is used.

The qualified data extent is five sectors beginning at absolute LBA 8: bytes
4096 through 6655 inclusive. All bytes outside that extent, including the
surrounding guard regions, are hashed before and after the experiment. Hash the
boot image independently. Capacity must exceed the extent sufficiently for the
runner's guards; IDENTIFY independently verifies that every granted sector fits.
The transport never accepts a guest-provided absolute address or controller ID.

`cache=writeback` leaves flushes enabled, and the explicit no-flush setting
documents the requirement. The current runner explicitly requests
`werror=report,rerror=report`; the final normal-path rerun passed with these
arguments independently observed in all eight QEMU processes. The retained initial
normal-path run preceded that option change and used QEMU defaults
(`rerror=report`, `werror=enospc`), under which host disk exhaustion could stop
the VM and make the runner fail its deadline. That receipt does not qualify
injected backend-error delivery or the updated command line. A later failure
injection run must record and qualify its actual policy. These settings are defined in
QEMU's [invocation documentation](https://www.qemu.org/docs/master/system/invocation.html).
They do not prove that the host filesystem or physical device honestly persists
data on flush. No test here is a physical host-power-loss experiment.

## Ownership and admission

`native_storage_device::device` owns one channel for the entire kernel boot.
Its unsafe `acquire(extent_start)` checks IF-clear state and extent arithmetic,
then consumes a global atomic claim without reading or writing any port. It is
move-only. Dropping it does not reset the claim or device. There is no release,
reset, unpoison, re-acquire or in-VM fencing operation.

The proof coordinator retains this owner while individual services are destroyed
and recreated. Only that controller creates grants tied to an admitted service
task/generation, the retained device generation, exact extent and allowed
operations. The editor has no device grant. Descriptor bytes and disk metadata
are information, never authority. Device poisoning survives service retirement
because it belongs to this retained owner.

The trusted coordinator invokes `identify` to qualify the exclusively claimed
device before creating application grants. Before task-bound read, write or
flush, the runtime completes its exact task, role, generation, handle, operation
and request-shape admission.
For writes it copies all 512 bytes to a private kernel buffer first. For reads
it preflights the entire destination's ownership, permissions and mappings
before issuing the device command. IF remains clear from admission through
copyout and reply; the single BSP and masked NMI assumptions remain unchanged.
No raw port, user pointer or physical alias crosses this interface.

The transport's own state/IF/extent checks are local and precede every port
access. Its complete-array parameter types require exactly 512 bytes. It stages
reads in a private 512-byte array and publishes that array only after successful
command completion. It clears the array using volatile stores on both success
and failure. Runtime copies have the same complete-copy requirement and must
clear their temporary storage separately.

## Qualified command subset

The task-file registers are `0x170..0x177`; alternate status/device control is
`0x376`. Data uses 16-bit accesses, other registers use 8-bit accesses. nIEN is
set to suppress channel interrupts; SRST is never asserted. There is no DMA,
PCI discovery, interrupt handler, 48-bit addressing, multiple-sector transfer,
ATAPI, transfer-mode negotiation or general device probing.

Only IDENTIFY DEVICE `0xec`, READ SECTORS `0x20`, WRITE SECTORS `0x30` and FLUSH
CACHE `0xe7` are issued. The command names/opcodes and IDENTIFY's capacity words
are also documented by the [Seagate PATA product manual](https://www.seagate.com/staticfiles/support/disc/manuals/notebook/momentus/7200.1/PATA/100320518c.pdf),
sections 4.1.1 and 4.1.2. This implementation is an emulator-qualified subset,
not a claim of general physical ATA timing compatibility.

IDENTIFY requires ATA rather than ATAPI, LBA support, a valid word 106 reporting
256 words per logical sector, supported/enabled FLUSH CACHE, and nonzero LBA28
capacity covering the entire retained extent. Invalid legacy word 106 is
rejected rather than interpreted heuristically. Physical-sector size does not
create a logical-sector or atomic-write guarantee. QEMU's
[IDE disk admission](https://github.com/qemu/qemu/blob/v10.2.1/hw/ide/ide-dev.c)
requires 512-byte logical sectors; the guest still validates returned geometry.

The selected master is checked for ready, not-busy state with no unexpected
DRQ. Before selection, a no-device status is tolerated because firmware may
leave the unpopulated slave selected. After selection, absent-device, ERR and DF
states fail closed. Each sector transfer uses count one and exactly 256 words.
DRQ/readiness is checked before each word, detecting an early transfer end;
completion requires BSY and DRQ clear, ready set and no reported error. Four
ordered alternate-status reads follow selection, issue and transfer completion.
These are a controller settling convention, not a measured wall-clock delay.

QEMU's [IDE command implementation](https://github.com/qemu/qemu/blob/v10.2.1/hw/ide/core.c)
provides the IDENTIFY geometry/flush fields, one-sector PIO path and asynchronous
flush completion used for this qualification. Its flush command remains busy
until the backend callback completes. A completed sector write alone does not
constitute the durable userspace save acknowledgement.

One budget of 1,000,000 alternate-status samples covers the entire operation,
including selection, all waits, transfer checks and settling reads. There is no
per-phase replenishment, PIT deadline, IF enable or unbounded busy loop. The
budget bounds guest work; host scheduling can still stretch wall-clock duration.
Evidence must retain actual issued/completed counts and timeout outcomes.

## Errors, poisoning and recovery

Before-I/O rejections preserve all device counters and the read destination.
They do not poison a healthy device. Once an admitted operation attempts device
access, any failure poisons the retained owner, including absent/unsupported
device, malformed geometry, error status, unexpected transfer and exhausted
budget. This is deliberately conservative even when no write command was issued.
The original error identifies the failing observation; all later operations
return `poisoned` before port access.

The runtime treats a failed device transaction conservatively and userspace's
durable store quarantines uncertain writes/flushes. It cannot convert timeout
into proof that a save did not reach disk. Rebinding a service or dropping a
borrow cannot restore the device. The first profile fences uncertainty only by
terminating the exact isolated QEMU process and creating a cold controller in a
new process, retaining the same exclusively owned data image. This fences that
emulator instance; it does not prove physical host storage quiescence.

Userspace `durable.rs` owns the header/checkpoint/checksum, formatting, epoch and
document policy. Formatting remains explicit and blank-extent-only. Recovery
must not use a fixture fallback. Remount advances the durable epoch before the
editor can open. Reopened bytes/version reconcile current contents; this format
does not persist per-request receipts or prove request history after a lost
reply. Disk rollback and backup restoration remain outside this gate.

## Feature-private completion-observation loss

The separately qualified `native_durable_failure_test` extension adds two
controller-only methods; normal builds contain neither them nor their state/branches.
`arm_flush_observation_loss() -> Result<(), storage_error>` requires healthy,
identified, IF-clear ownership and rejects a repeated pending arm. It reads no
port. Only the trusted proof runtime may call it after validating the exact
service and delivered target save for case 7. It is not an arbitrary userspace
device operation or a field of disk metadata.

The arm counts subsequent FLUSH commands actually issued, not writes, reads or
gate attempts. The body FLUSH completes through the ordinary mechanism. On the
second FLUSH, the command-register write and issued counter occur first; then
the remaining poll budget is set to zero before `finish` can sample alternate
status. Ordinary exhaustion handling returns `poll_exhausted` and poisons the
retained owner. Completed-command and successful-flush counters do not advance
for that command. There is no further port access while propagating the error.

This is deterministic loss of completion observation after a real command. It
does not simulate spontaneous hardware ERR, claim that the device was still
busy, or measure a real timeout. The device may already have completed its
flush; the caller has not observed completion and must retain unknown outcome.
An earlier actual command failure also poisons the owner and cannot be cleared
through the arm method.

`grant_available() -> bool` is a local preflight, not a grant constructor. It
requires identified, healthy, IF-clear state with no pending injection, and
performs no port access. The runtime still validates current service identity,
generation and exclusive binding. After poisoning, original-service valid
read/write/flush calls reject without I/O. Ending the service borrow, destroying
both task owners or attempting successor admission leaves the same device
poisoned; successor grant creation and remount must be denied. Only the separate
cold-VM lifecycle can begin with a new owner. No driver reset is implied.

Four feature-scoped mock tests independently count reads and writes at command
issue, prove zero completion sampling after the selected FLUSH, verify normal
first-flush completion and unrelated transfers, and check permanent no-I/O
denial across service borrow lifetimes. The target proof must separately show
real task cleanup, store quarantine, no false clean editor and cold recovery.
These tests and the separate native extension passed. Its 28 debug-exit boots
cover seven cases at two memory sizes with initial and cold execution; fourteen
additional host-quit captures verify the public editor region. Cold input carries
no answer, and the host retains untouched pre-recovery media. The
[failure log](../../notebook/Development/2026-10-04%20Native%20Durable%20Failure%20Log.md)
binds the exact artifacts, complete stack/ownership review, all required checks,
regressions and memory limits. The normal-path evidence below remains a distinct
qualification and is not the basis for the extension's failure claims.

## Review and evidence

The initial QEMU q35/TCG/Nehalem run passed provisioning/save plus full pair
destruction/remount, then cold recovery in a new VM process, at both 96 MiB and
320 MiB. Each size used a separate 1 MiB disposable disk. The host independently
verified saved challenge bytes, version/epoch progression and every byte outside
`[8,13)`, while checking that the boot image remained unchanged. Recovery input
contained no expected answer. A separate host-quit capture verified the entire
9,216-pixel application region.

Blank recovery, corrupt-header recovery and attempted reformat of nonblank
media produced the expected failure exits without changing any data-image byte.
These demonstrate refusal of invalid media; they do not exercise native commit
cuts, lost replies, device timeouts or poisoned-device remount. The initial
receipt and exact artifact hashes are recorded in the
[development log](../../notebook/Development/2026-10-04%20Native%20Durable%20Document%20Log.md).
All 16 required checks, native/ELF replacement and workspace regressions passed.
The final native artifact is bound to the emitted stack and boundary review
through complete executable/data comparison; only panic source-line metadata
changed after the initial review. See the development log for exact hashes.

`io_counters` counts every byte or word port access, plus issued/completed
commands, successful sector reads/writes and successful flushes. Counters
saturate defensively; the finite proof must stay far below saturation. Port
counts include IDENTIFY, status polling and control/task-file writes, so a
command-count-only denial assertion is insufficient.

The safe generic transport has mock-port tests for claim uniqueness, overflow,
unidentified/IF-enabled/out-of-extent rejection, geometry admission, exact sector
commands, short read/write, failed completion, finite post-issue polling and
permanent poison. Independent mock counters check every port access. These
tests do not simulate persistent media, real ATA timing or host power failure;
the portable durable crash model and actual cold-VM proof supply distinct
evidence.

The target review must bind the exact kernel and verify the four port-I/O
instruction wrappers, unchanged privilege-entry/CPU-state rules, IF-clear call
sites, complete user-copy preflight, retained owner lifetime, bounded loops and
stack depth with the added 512-byte staging array. Parent-run serialized checks
must qualify the provisioned controller and preserve all earlier RAM/native/ELF
proofs before accepting this temporary mechanism.

The architectural destination remains a supervised userspace driver with a
sealed device grant, explicit I/O-port delegation or another reviewed transport,
and qualified cancellation/reset/fencing. Moving these opcodes to userspace
without that ownership and fencing work would not complete the boundary.
