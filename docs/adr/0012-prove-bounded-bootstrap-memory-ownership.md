# ADR 0012: Prove bounded bootstrap memory ownership

- Status: Accepted
- Date: 2026-09-09

## Context

Before this decision, Threshold handed the kernel a bounded BIOS E820-shaped
table that the kernel used only to produce a raw summary. That summary could
double-count overlaps, include partial and inaccessible ranges, and could not
distinguish a complete enumeration from one stopped by a firmware error or the
64-entry capacity. It was not safe allocation authority.

The fixed circle experiment in ADR 0011 proved selected x86-64 paging and
teardown behavior with statically linked storage. It deliberately did not
define general frame ownership, address spaces, or reclamation. Reusing its
fixed pages and test gate as permanent machinery would turn an experiment into
ABI without first establishing who owns each physical and virtual range.

The next proof must therefore establish a small ownership core before adding a
heap, scheduler, task model, production circles, or unprivileged shell. It must
remain allocation-free while it bootstraps the allocator itself.

## Decision

### Scope

Implement a bounded x86-64 bootstrap memory manager for the bootstrap processor
using 4 KiB frames below the current four-GiB identity-map limit. The normal
kernel initializes this manager from a kernel-owned copy of the handoff table.
A separate `memory_test` image exercises allocation, two managed address-space
mappings, withdrawal, reclamation, and clean reuse.

The physical state ledger has one two-bit state for every currently addressable
frame and resides in zero-initialized kernel memory. All other capacities are
fixed and checked. Exhaustion is an explicit error and must not leave partially
mutated ownership state. The implementation is kernel private and pre-alpha;
its Rust tokens, test address, event names, and paging layout are not stable ABI.

### Complete handoff before trust

The memory-map contract becomes incompatible version 2 even though the
88-byte record layout remains unchanged. Threshold sets a dedicated
`memory_map_complete` feature flag only after BIOS enumeration reaches its
defined end with `EBX=0`. Firmware carry, a bad signature, a short result, or a
nonzero continuation at the fixed 64-entry capacity is a human-visible
Threshold boot failure before kernel entry. The kernel independently rejects a
map unless the flag is present, providing structured and VGA-text diagnostics
for a malformed or alternate handoff that nevertheless calls it.

Version 2 descriptors retain the bounded 24-byte `{base, length, kind,
attributes}` shape as the canonical representation for this handoff version.
Threshold translates firmware discovery into that representation; a future
UEFI frontend must do the same rather than expose UEFI policy to the kernel.
Only kind 1 with exactly the enabled, volatile attributes is an allocatable
candidate. Disabled records grant nothing. Unknown, reclaimable, nonvolatile,
bad, and every other kind remains unavailable until a later owner explicitly
claims it.

Version 2 may be superseded and makes no source or binary stability promise to
interfaces other than this exact Threshold-to-kernel handoff contract.

The kernel copies all accepted descriptors into fixed kernel storage before
the handoff buffer could become reclaimable. The raw table is never retained as
live allocation policy.

### Conservative normalization

Normalization produces page-level ownership independent of firmware ordering:

- enabled `base + length` overflow rejects the complete map;
- ranges are clipped to the supported four-GiB ceiling;
- usable candidates align inward to whole 4 KiB frames;
- unavailable and kernel-reserved ranges align outward;
- every enabled unavailable overlap takes precedence over a usable record;
- duplicate usable coverage is counted once; and
- failure rejects initialization without publishing a partial allocator.

The bootstrap reservations are:

- all physical memory below one MiB, covering the BIOS path, manifest, stage 2,
  staging buffer, handoff records, active loader page tables, and loader stack;
- the linker-defined `[kernel_image_start, kernel_memory_end)` extent; and
- the complete validated framebuffer aperture.

The linked kernel extent must itself be covered by normalized volatile usable
memory before it is reserved. A framebuffer overlapping low bootstrap memory or
the linked kernel is rejected before drawing. Gaps in the firmware map are not
memory.

### Physical-frame ownership

The manager owns every allocatable frame and grants only whole frames through
an opaque `frame_lease`. A lease identifies its allocator instance, private
ledger slot, generation, extent, and data-or-table purpose. Allocator-instance
identities never wrap, so a token from an earlier or different manager cannot
authorize mutation. Callers cannot release by raw address. Stale, altered,
duplicate, foreign, unknown, or still-mapped leases are rejected.

Allocation is deterministic lowest-address first with explicit frame count and
power-of-two alignment expressed in frames. Zero, non-power-of-two, overflowing,
and unsupported alignments fail before mutation. A frame has at most one
exclusive lease; second managed aliases and sharing are not supported in this
slice. Lease generations never wrap: a ledger slot whose next generation would
wrap is exhausted rather than allowing an old token to become current. Before a
lease is granted, its frames are cleared and verified through the current
identity mapping. Before a released extent becomes available again, it is
cleared and verified. Failed clearing quarantines the active lease rather than
making residue allocatable. The allocation, release, and raw physical-clear
interfaces are explicitly unsafe: their caller must hold exclusive authority
over every byte passed to the volatile clear operation.

These tokens are internal bookkeeping, not sigils, process identities, or
security objects that can cross a kernel boundary.

### Managed virtual ownership

A fixed-capacity address-space owner consumes a complete live frame lease when
binding it to one page-aligned canonical virtual extent and returns that same
noncopyable lease only after the leaf is removed and the allocator binding is
withdrawn. Bindings reject overlap, a second alias of the same lease, count
mismatch, and writable plus executable permissions.
This W^X rule applies only to mappings created by the new manager; Threshold's
existing four-GiB identity map remains a documented writable and executable
bootstrap limitation.

The feature proof creates two allocator-backed private roots. Each borrows the
existing supervisor-only low kernel branch and owns every new high branch and
leaf. Both bind the same test-local virtual address to distinct owned data
frames as supervisor RW+NX. Exact table walks must match the ownership records
before CR3 may select either root. Switching A to B to A must preserve two
distinct marker values at the same virtual address.

NX support, `EFER.NXE`, and supervisor `CR0.WP` are required and read back
before managed mappings become active, then remain enabled as normal kernel
protections. These processor controls move into shared architecture code rather
than remaining a circle-test assumption.

Teardown must restore the original CR3, remove both virtual bindings, destroy
the inactive roots, and only then clear and release data and page-table leases.
The owner is the validation and ownership record for these page-table edits; it
is not yet a general mapper, user address-space API, or circle.

### Observable lifecycle

Normal boot emits a structured `kernel.memory.ready` record containing bounded
counts and limits, and presents a concise memory-ready state on both packed-pixel
and VGA-text boot surfaces. It adds no bootstrap-shell command or rite.

`make memory-smoke` builds a feature image and requires exactly one ordered
lifecycle:

1. a complete handoff is copied, normalized, and published;
2. allocator-selected data and page-table leases are granted zeroed;
3. two private roots with non-W+X bindings are walked and activated;
4. A-to-B-to-A switching proves distinct physical ownership behind one virtual
   address;
5. the original CR3 is restored before either mapping or root is withdrawn;
6. released data leases are reacquired and observed as zero;
7. every lease is released and the initial free-frame count is restored; and
8. the test reports success through the test-only QEMU exit device.

The QEMU proof runs with at least two RAM sizes so its available-frame evidence
cannot be a single hard-coded machine assumption. A normal-image inspection
rejects the test events and entry path. The dedicated VirtualBox VM reproduces
the complete lifecycle before its normal interactive image is restored.

## Required invariants

- An incomplete, malformed, wrapping, or over-capacity map grants no frames.
- Non-usable firmware information and explicit reservations always win overlaps.
- Free frames are below four GiB and disjoint from low memory, the kernel, and
  the framebuffer.
- Every granted frame belongs to exactly one live generation-checked lease.
- A physical lease cannot be released while a virtual binding names it.
- Every managed page-table frame has a live owner until its root is inactive.
- Managed mappings are never simultaneously writable and executable.
- CR3 no longer names a root before any of its frames can be reclaimed.
- Reissued frames contain zero, including after stale and invalid release
  attempts.
- Human and machine boot projections agree when kernel memory initialization
  succeeds; Threshold and kernel rejection paths remain explicitly attributable
  at their respective pre-kernel and kernel stages.

## Non-goals

This decision does not define or implement:

- a Rust heap or general-purpose dynamic allocation API;
- stable frame, mapping, or address-space ABI;
- general circles, vessels, tasks, threads, scheduling, syscalls, or IPC;
- shared memory, aliases, copy-on-write, demand paging, swapping, or files;
- high memory, huge pages, NUMA, hotplug, or memory above four GiB;
- multiprocessor locking, TLB shootdown, PCID, or concurrent teardown;
- pinning, DMA ownership, an IOMMU, or accelerator placement;
- global kernel W^X, KPTI, SMEP, SMAP, ASLR, or side-channel resistance;
- reclaiming Threshold's low-memory pages; or
- Reliquary, Covenant, Canticle, exec-hag, package, or shell integration.

## Security limitations

The current identity map still aliases managed physical frames into a writable
and executable supervisor range. Logical ownership and verified clearing do not
isolate a compromised ring-0 caller. The proof runs with interrupts disabled on
one processor, cannot preempt a looping context, and does not address DMA or
speculative leakage.

The BIOS and hardware remain trusted to return a truthful complete map after
the structural checks. Version 2 detects incomplete enumeration but does not
authenticate firmware claims. Physical addresses may appear only in the local
feature-test channel and remain subject to the diagnostic disclosure limits in
the threat model.

## Acceptance evidence

Host tests must cover structure layout, complete and incomplete handoffs, zero
and disabled entries, unknown and nonvolatile types, range overflow, clipping,
inward and outward alignment, unordered overlap precedence, duplicate coverage,
every explicit reservation, kernel coverage, framebuffer conflict, and failure
without partial publication.

Ownership tests must cover deterministic allocation, alignment and exhaustion,
unique leases, altered and stale generations, duplicate release, mapped-frame
release, virtual overlap, duplicate physical binding, count mismatch, W^X
rejection, active-root destruction, and exact free-count restoration. Page-table
tests must distinguish owned from borrowed branches and walk exact physical and
effective permissions. Volatile clearing and zero-before-reuse require both
host-visible behavior and emulator evidence.

Acceptance additionally requires `make check`, the two-size
`make memory-smoke`, normal-image exclusion, all existing boot/fault/circle/shell
regressions, and the same feature lifecycle under VirtualBox.

All gates passed on 2026-09-09:

- `make check`, normal-image exclusion, and the boot, fault, circle, and shell
  smoke suites completed successfully.
- At 96 MiB, QEMU reported 24,162 free and 1,024,414 unavailable frames. At
  320 MiB it reported 81,506 free and 967,070 unavailable frames. The exact
  57,344-frame increase equals the 224 MiB RAM increase.
- Both QEMU runs selected data frames `0x17d000` and `0x17e000`, table roots
  `0x17f000` and `0x183000`, proved distinct markers across A-to-B-to-A CR3
  switching, and emitted explicit `overlap_rejected`,
  `duplicate_binding_rejected`, and `active_root_rejected` evidence before
  restoring the initial free count after zeroed same-frame reuse.
- The dedicated 512 MiB VirtualBox VM reproduced the complete lifecycle with
  eight normalized regions, 130,675 free and 917,901 unavailable frames, the
  same selected extents and markers, and no error- or fatal-level record. Its
  evidence remains in the ignored `build/virtualbox-memory.log` artifact.
- The normal image was then restored. At that checkpoint it reported 130,686
  free frames, emitted `boot.ready` and `bootstrap_shell.ready`, and was observed
  running in the dedicated VM.

Independent review found and the accepted implementation corrected
allocator-instance provenance, unsafe raw-clear authority, ownership transfer
for mapped leases, fail-closed framebuffer parsing and reservation, strict
transcript prerequisite and error validation, and human-visible framebuffer
failure diagnostics. Threshold requires VBE 2.0 or newer, selects the legacy
pitch and RGB layout for VBE 2.x or the linear layout for VBE 3.x and newer,
requires supported graphics and linear-framebuffer mode attributes, and
validates geometry, extent, channel bounds and separation, and occupied-memory
separation before graphics activation or publication. Rust independently
enforces that RGB masks fit the declared bit depth and do not overlap.

Direct BIOS E820 fault injection and repeated execution of singleton allocator
publication are not part of the host suite: firmware discovery and one-time
global state cannot be faithfully reset there. The loader branches and
publication transition are covered by code review; QEMU and VirtualBox execute
the production initializer, while kernel tests and emulator gates exercise its
fail-closed handoff boundary. This does not weaken the continuing firmware-
truthfulness assumption or the single-publication, bootstrap-processor-only
scope.

## Consequences

The kernel gains its first conservative authority over ordinary physical frames
and a narrow rule connecting that authority to virtual mappings. Future page
tables, heaps, tasks, circles, DMA buffers, and shared memory can build on
explicit ownership rather than raw addresses.

The cost is additional kernel state, unsafe physical-memory clearing, stricter
firmware requirements, and another architecture-specific test path. Fixed
capacities and lowest-address allocation trade flexibility for inspectability
during bring-up and may be replaced behind later ADRs.

## Alternatives considered

### Allocate directly from type-1 E820 records

This is unsafe in the presence of truncation, overlap, partial pages, occupied
kernel memory, framebuffer apertures, and duplicate records.

### Keep boot_omen version 1 and reinterpret an unused flag

Requiring complete enumeration changes which handoffs are trusted. An explicit
version change makes that incompatibility visible even though the record layout
does not grow.

### Add a heap immediately

A heap would consume frames before physical ownership, clearing, and failure
atomicity were established. It follows this proof rather than hiding it.

### Generalize the fixed circle page tables

ADR 0011 intentionally made those pages, addresses, gate, and recovery
trampoline removable. The new proof allocates its own resources and borrows only
the already-live supervisor kernel branch.

### Define circle or sigil identity here

A circle combines memory and authority; a sigil conveys authority. This slice
establishes internal memory bookkeeping only and must not preempt those later
security designs.
