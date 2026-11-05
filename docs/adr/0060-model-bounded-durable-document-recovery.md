# ADR 0060: Model bounded durable document recovery

- Status: Proposed; portable model qualified, native device integration pending
- Date: 2026-10-04
- Related: ADR 0004, ADR 0013, ADR 0045, ADR 0056, ADR 0058, ADR 0059

## Context

The native and ELF replacement proofs retain one RAM document service. They
recover saved content after editor loss, but destruction of that service loses
the document. The next persistence gate needs a precise durable acknowledgement
and recovery contract before a device adapter can implement it.

There is no runtime block driver in the present kernel. Threshold's BIOS disk
reads precede long mode and cannot service application writes. This checkpoint
therefore implements an allocation-free, safe `no_std` userspace model over an
explicit sector interface. It does not add a kernel gate, modify the qualified
RAM service, or claim that a guest save already survives a cold VM boot.

## Decision

Add a separate `durable` module in `anomalous-document-protocol`. The adapter
supplies exclusive access to five 512-byte sectors and read, write and flush
operations. Its successful flush must make preceding successful writes survive
the restart being modeled. Writes may be torn or uncertain on failure. The
adapter must preserve sectors outside a write's addressed sector; the model
does not cover arbitrary disk corruption, dishonest flushes, or media rollback.
Before reusing a failed adapter, its previous commands must have completed,
been aborted/reset, or otherwise been fenced. Dropping the Rust store alone
cannot prevent a delayed timed-out write from overwriting a reused slot.

Formatting is a deliberate operation on a dedicated disposable extent, allowed
only after all five sectors are verified zero. Recovery never silently formats
invalid media. An immutable canonical header binds the format, geometry and
volume/object identity. Two alternating body/commit pairs hold sequence, mount
epoch, content version and at most 31 printable ASCII bytes. Integrity checks
detect accidental corruption, not hostile modification or authority. The
experimental layout is versioned but is not a stable filesystem format.
Recovery guarantees assume torn/corrupt records are detected; CRC32C collisions
are explicitly excluded. Prefix-tear tests do not enumerate every possible
corruption pattern.

A transition writes the inactive body, flushes, writes its matching commit
record, then flushes again. It leaves the active pair intact. Recovery accepts
only a complete canonical matching pair and selects the newer sequence. It
rejects ambiguous equal-sequence records, foreign identity and invalid geometry.
Recovery can read an exhausted valid checkpoint; mount/save reject further
counter advancement without writes. A partially prepared inactive pair cannot publish
mixed content or erase the last acknowledged snapshot under the stated
device contract.

Persistent object identity and content version are distinct from the wire
document incarnation. Mount durably increments an epoch before admitting a
client, then uses that epoch as the wire incarnation. Handle generations,
request cursors and editor revisions remain volatile. The embedding service
must authenticate and exclusively bind the route to this volume/object; an
epoch or disk checksum supplies no task authority. Fresh mounts reject stale
wire documents even when VM task-generation counters restart.

The wrapper preflights the existing document request against a candidate RAM
state. A semantic rejection performs no I/O. For a save, it publishes the
candidate and releases `saved` only after the final flush succeeds. Any
uncertain I/O quarantines the store and returns an outcome separate from
semantic rejection. Further calls cannot reinterpret the uncertainty as a
stale request or version conflict; the caller must preserve the pending save
and recover. A reply lost after the final flush may leave a newer recoverable
snapshot even though the client cannot yet know the outcome.
The 128-byte document wire format is unchanged. Durability belongs to this
explicitly selected service/adapter contract; a `saved` record from the existing
RAM service still has only RAM semantics. Later native startup/route policy must
make that distinction explicit rather than inferring durability from the opcode.

## Qualification

Keep the qualified native/ELF clients and RAM semantics unchanged. Unit tests
cover format validation, authority, versioning, canonical encoding, counter
exhaustion and write-free rejection. An independent durable/volatile-sector
model injects restart or I/O failure at transition boundaries and torn sector
prefixes. It compares recovered content against known old/new snapshots,
checks that acknowledged saves survive, and rejects stale mount identities.
It must not derive expected document bytes by invoking the implementation's
record codec. Run all required repository regression checks under the existing
memory limits and retain the exact tested source and results.

VirtualBox qualification is separate: run the already qualified normal and
native/ELF images with isolated VM ownership, bounded lifetime and serial logs.
A terminal event is only a capture trigger; the strict existing transcript
oracle and complete frame comparison decide application proof acceptance.
Do not infer host readiness from direct access to `/dev/vboxdrv`: installed
launchers may mediate that access. Record actual startup success or failure.

## Next hardware boundary

A later increment must define and qualify device authority before linking this
model into a live service. A candidate is a feature-scoped one-sector
ATA PIO transport on a dedicated legacy IDE data channel, with the boot disk
kept separate. It needs exact task-generation and extent checks before port I/O,
complete checked copies, device/sector/flush validation and bounded polling.
Current gates run with interrupts disabled, so PIT ticks cannot provide a
polling deadline. An exhausted poll budget after issuing a write or flush means
unknown outcome and requires quarantine. This proposal does not yet authorize
a permanent kernel driver or general raw-port API; userspace driver isolation
remains the direction of ADR 0004.

The live proof must destroy the complete service and recover through a new
owner, then cold-start a new VM against the same separately retained test disk.
It must retain explicit emulator flush settings: VirtualBox guest IDE/SATA
flush handling requires the correct data LUN's `IgnoreFlush=0`, and QEMU must
not suppress flushes or use unsafe/snapshot storage. See the
[VirtualBox flush setting](https://docs.oracle.com/en/virtualization/virtualbox/7.2/user/Troubleshooting.html)
and [QEMU block options](https://www.qemu.org/docs/master/system/qemu-manpage.html).
Neither cold guest restart
nor a host memory model proves physical host-power-loss resilience.

Full filesystem naming, directories, multiple objects, concurrent writers,
general process management, full Antimatter/Electron and additional foreign
personalities remain separate gates.
