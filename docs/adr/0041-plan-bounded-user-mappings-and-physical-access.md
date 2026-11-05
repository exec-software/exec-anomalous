# ADR 0041: Plan bounded user mappings and physical access

- Status: Proposed
- Date: 2026-09-26

## Context

The [execution-readiness audit](../architecture/execution-readiness.md) identifies
address geometry and privileged physical access as prerequisites for native
application launch and a later Linux ELF adapter. The current
[address-space owner](../../kernel/src/arch/x86_64/address_space.rs) accepts three
extents in one leaf-table branch, borrows supervisor PML4 slot zero, and rejects
user mappings anywhere in that slot. The kernel is linked at one MiB; page-table
access and physical clear/copy paths still cast physical addresses to pointers
through the bootstrap identity map.

Removing the slot-zero check would therefore do more than permit a typical low
executable address: a user mapping could redirect privileged accesses through
the same virtual address. Raising descriptor capacity would not solve that
ownership hazard. Accepted
[ADR 0012](0012-prove-bounded-bootstrap-memory-ownership.md) requires genuine
allocator leases, independent permissions, withdrawal before reuse, and verified
clearing. No address calculation can replace those obligations.

## Proposed decision

Add two kernel-private, allocation-free, unsafe-free planning components:
[`mapping_plan`](../../kernel/src/arch/x86_64/mapping_plan.rs) validates bounded
user virtual geometry and resource demand;
[`physical_window`](../../kernel/src/arch/x86_64/physical_window.rs) validates
the arithmetic of a candidate high-half physical window. Their outputs are inert
descriptions. Neither component allocates memory, accepts or creates a lease,
constructs a pointer, populates page tables, writes CR3, grants access, or launches
code. They introduce no wire format, ELF parser, or selected production layout.

### Bounded user geometry

Accept an explicit finite set of mapped extents with requested permissions and
an explicit finite set of excluded virtual extents for supervisor reservations
or guards. The experiment accepts at most eight mappings and eight lower-half
exclusions, with at most 32 user-tree table frames including one root. Caller
data-page and table-frame budgets are independent. Validate every descriptor
before producing a plan; unused or distant reservations do not excuse malformed
input. These capacities are internal proof limits, not application ABI.

Use the existing four-level x86-64, 4-KiB-page model. Every extent is nonempty,
page aligned, and checked as a complete half-open range. Mapped user ranges stay
within the lower canonical half; an exclusive end at its limit is valid, but no
included byte may enter the canonical hole. Exclude mapped page zero while
allowing a reservation beginning there. Requesting supervisor permissions or
writable executable user memory fails. Do not infer permissions from placement
or turn requested rights into authority.

Reject overlap between mapped extents even when permissions agree, and reject
any mapping intersecting an excluded supervisor or guard extent. Adjacent
nonoverlapping ranges remain distinct valid inputs. Exclusions may overlap each
other and represent their union; they allocate no pages. The caller must
explicitly describe the protected geometry; planning a low user range is not
evidence that the running kernel has relocated or that its reservations are
complete.

Account for mapped data pages separately from page-table frames. For 4-KiB
leaves, count one root plus the distinct PML4, PML4/PDPT, and PML4/PDPT/PD
prefixes needed by mapped pages. Segments sharing a branch share its table cost;
sparse distant segments incur only their actual branches rather than the entire
intervening address span. This counts a new user tree, not the complete future
root including supervisor/direct-map tables; merged ownership needs a later
independent budget. Check whole-range data demand before enumerating 2-MiB
branches. Each new branch needs a unique table, so the table limit bounds work
without a per-page loop. Check arithmetic, descriptor capacity, and both
independent budgets without partial publication. Accepted mappings are sorted
and immutable, with unique full branch prefixes retained.

Retain per-extent requested permissions and exact membership information. An
address in a gap between executable extents is not executable merely because it
lies between their minimum and maximum addresses. This evidence does not replace
the live task's current executable-bound or resumed-stack validation, prove that
code was initialized, or authorize entry to it.

### Checked physical-window geometry

Describe a candidate physical aperture and a distinct typed virtual aperture in
the canonical high half. Validate nonempty aligned ranges, representable checked
ends, the page-entry format's 52-bit physical envelope, canonical coverage, and
equal span length. This encoding bound is not proof of the processor's physical
address width or available memory. An exclusive end at 2^64 is unrepresentable
and rejected. Translation in either direction accepts nonempty byte subranges,
validates their complete inclusion, and performs checked offset arithmetic.
Low virtual user addresses must not be confused with physical addresses simply
because their numbers match.

The descriptor proves arithmetic only. It cannot establish that any frame is
RAM rather than a hole or MMIO, owned, mapped, accessible under the active root,
cache-compatible, cleared, or protected by effective permissions. It must not
return a dereferenceable pointer or expose an activation API. It chooses no
permanent kernel base, direct-map base, address-space layout, or boot protocol. A future
resident kernel, transition stack, tables, devices, and window must have a
separately proven nonconflicting placement.

## Validation and disproof

Host tests must cover checked overflow, zero/unaligned extents, both canonical
boundaries, mapped page zero, reservation and guard collisions, exact adjacency,
W^X, supervisor requests, descriptor exhaustion, and each budget's exact boundary.
Exercise crossings at 2 MiB, 1 GiB, and 512 GiB, shared branches, sparse layouts,
and input-order independence. An independent bounded host oracle should enumerate
pages into sets of distinct prefix identities rather than reuse the production
counting routine. Check exact executable membership including gaps and edges.

For physical-window geometry, test high-half and physical-limit edges, arithmetic
overflow, the complete requested subrange, first/last included pages, exact
exclusion at the end, invalid aperture construction, and offset round trips
including byte subranges in both directions. Rejection leaves caller inputs and
previously accepted descriptions unchanged. These tests prove planning rejection
atomicity, not allocation rollback, stale-TLB safety, or memory reclamation.

The implementation supplies 11 mapping-planner tests with an independent bounded
`BTreeSet` page oracle, and eight physical-window tests. Its four-region example
contains seven data pages and nine user-tree table frames: one root, five tables
for the low regions, and three tables for the distant stack. Those counts are
planned demand, not allocated frames or a running application.

Focused validation passed: `cargo test -p kernel --lib arch::x86_64` ran 70 tests,
including all 11 planner and eight physical-window tests. Freestanding
`cargo clippy -p kernel --target x86_64-unknown-none -- -D warnings` passed.
Independent code review found no actionable issue. The complete existing
23-target host/native regression matrix passed in 1,089.4 seconds of combined
target time; all 230 frozen source/configuration files remained unchanged.
Full kernel suites passed 361 default and 404 all-feature tests. TCG and KVM
both retained the exact dispatcher and workspace proof outcomes. The normal,
dispatcher, and workspace images are byte-identical to their pre-change images;
the [development log](../../notebook/Development/2026-09-26%20User%20Mapping%20Plan%20Log.md)
records hashes and final evidence links.

Existing runtime behavior should remain unchanged because no live mapper consumes
the plans. Native regression success is evidence against regression, not evidence
that the proposed physical window or low user mappings were activated. No boot
or live-mapper change is introduced, and no new VirtualBox run is claimed.

## Consequences

The next mapper implementation gains independently checked geometry and demand
before it stages ownership changes. A later native proof must still establish a
privileged access path valid under every active root, lease-backed tables and
data, effective permission validation, failure-atomic allocation/population,
activation and TLB rules, and complete withdrawal/clearing/reclamation. The
current slot-zero restriction and physical-as-pointer runtime remain intact.

This increment does not complete low-address Linux ELF execution, general user
copy, multiple executable task bounds, a native launch ABI, a high-half kernel,
or production memory isolation. Linux and later Mach-O adapters remain outside
the kernel and must eventually produce the same native launch description.

## Alternatives

- **Remove the low-branch reservation now:** rejected because current privileged
  physical accesses depend on that identity mapping.
- **Only increase current mapping capacity:** insufficient; more records do not
  create additional owned table branches or safe physical access.
- **Choose and activate a high-half layout in this increment:** deferred until
  resident-range, boot/interrupt, ownership, and teardown proofs are ready.
- **Parse ELF directly into kernel page tables:** rejected because format policy
  belongs in an adapter and parsed intent is not mapping authority.
