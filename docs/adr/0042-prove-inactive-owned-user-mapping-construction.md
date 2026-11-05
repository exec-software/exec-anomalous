# ADR 0042: Prove inactive owned user mapping construction

- Status: Proposed
- Date: 2026-09-26

## Context

[ADR 0041](0041-plan-bounded-user-mappings-and-physical-access.md) establishes
inert user geometry and physical-window arithmetic. Its plans own no frames,
install no translations, and cannot select a root. Accepted
[ADR 0012](0012-prove-bounded-bootstrap-memory-ownership.md) establishes the
allocator's actual frame leases, bound ownership, verified clearing, and reuse
rules. The next bounded proof should join these contracts without activating a
low user root while the kernel still executes through low resident mappings.

The existing mapper continues to reserve PML4 slot zero. Its physical pointer
paths depend on the bootstrap identity map, so a new user hierarchy containing
low addresses cannot replace the running kernel root safely. Construction and
verification of an inactive hierarchy can nevertheless exercise real ownership,
physical access, population, and recoverable teardown before that transition.

## Proposed decision

Add an internal `staged_mapping` owner in
[`owned_mapping.rs`](../../kernel/src/arch/x86_64/owned_mapping.rs). It consumes an
immutable mapping plan and stages actual allocator-owned storage: one contiguous
page-table arena of exactly the planned table count, and one contiguous data
lease for each mapped region. The plan's geometry and permissions remain intent;
the allocator grants every physical extent independently. Descriptor, data, and
table bounds remain explicit and checked.

`staged_mapping::new(plan)` creates the retained owner. Its unsafe `build`,
`validate`, and `cleanup` methods borrow that owner and an allocator/backend;
the unsafe `mapping_memory` contract provides whole-range `clear_and_verify`,
bounded `read_entry`/`write_entry`, and the actual `active_root`. The native
implementation is
[`bootstrap_physical_access`](../../kernel/src/arch/x86_64/physical_access.rs).
Safe `root_address`, `data_range`, `is_ready`, and `retained_leases` accessors
expose inspection metadata, never activation or physical access authority.

The owner retains every successfully allocated lease immediately, before a
subsequent fallible bind, population, verification, or cleanup operation. Staging
uses a caller-retained mutable owner rather than losing partially acquired
resources through an error return. No error may discard the only live handle,
invent replacement authority from a physical address, or make owned residue
available for reuse. Initial allocation clearing retains ADR 0012's callback
contract; a rejected allocation does not create a lease.

Populate only the planned four-level 4-KiB user hierarchy, children before
parents. Parent tables refer to exact owned table frames, leaves refer to the
corresponding owned data regions, and effective permissions match the plan's
user/read-write/execute intent. Validate the whole expected hierarchy, including
unused entries, exact frame extents and purposes, unique table/data ownership,
guards and gaps, and
the absence of unplanned mappings. Compare all 512 entries of every table against
the exact expected value; even accessed/dirty additions are rejected because
these trees have never been active. Independent host/native walks check actual
translations and effective permissions. A broad min/max executable interval
cannot replace exact region membership.

These roots remain **inactive**. The new owner exposes no activation or launch
operation. Construction, validation, corruption injection, and teardown must
never select its root in CR3. It provides no kernel execution mappings and is
not a runnable address space merely because its user leaves are well formed.

### Retryable withdrawal and clearing

Before unbinding or releasing staged storage, clear and verify the complete
owned table arena. The inactive-root invariant is a precondition: clearing an
active or still-used hierarchy would invalidate execution or free reachable
frames. A failed whole-arena clear keeps every retained handle and existing
ownership binding quarantined for explicit retry; it cannot report success or
continue reclamation.

After verified table withdrawal, unbind and release data and table leases using
the allocator's existing ownership checks and complete clear/verify contract.
Retain handles and accurate progress if a later release fails. A retry must
neither repeat an already completed release nor lose an unbound but still-owned
lease. Successful cleanup restores the starting accounting and leaves no live
arena/data handle in the stage owner. This is recoverable staged cleanup, not a
claim that every failure immediately returns every frame or that clearing cannot
fail. Active-root or foreign-allocator preflight rejection leaves the owner
unchanged, including prior readiness. Once mutating cleanup begins, a failure
keeps it withdrawing and unready until cleanup succeeds. Completed cleanup permits
rebuilding the same plan; there is no fallible automatic `Drop` cleanup.

### Temporary privileged physical access

Extend the removable `memory_test` proof with a temporary supervisor RW+NX
physical alias while keeping the existing kernel root selected during the new
staging phase. Validate the complete existing bootstrap hierarchy for physical
`[0, 4 GiB)` before borrowing it into an empty reserved PML4 slot. The proposed
test placement is `0xffff_9000_0000_0000` at slot 288; it selects no permanent
kernel/direct-map layout or boot protocol.

Installation must preserve the original low resident paths and establish exact
effective supervisor RW+NX permissions for the alias. The alias borrows the
validated bootstrap tables; it does not acquire ownership of them or authorize
their reclamation. The bridge validates Threshold's exact six-page contiguous
tree: a root with only slot zero populated, the complete PDPT with four present
entries, and all 2,048 huge-page leaves. NX applies only at the added root entry,
not by changing shared children needed by the low execution path.

Keep this proof serialized on the bootstrap processor. Reject IF enabled and
CR4 PGE, PCIDE, or LA57 before installing the bridge; it assumes no global/PCID
translations and the existing four-level geometry. Flush local translations by
reloading CR3 immediately after installation and withdrawal, before subsequent
access relies on the change. The ordering follows the
[AMD64 Architecture Programmer's Manual, Volume 2](https://docs.amd.com/v/u/en-US/24593_3.44_APM_Vol2).
Withdraw the alias only after every staged owner has cleaned up and no callback
or borrowed access can still use it, then verify the original root/slot state.

Window geometry proves that a translation fits, not that a range is RAM, safe
MMIO, allocated, or owned. The allocator's exclusive callback or a live validated
lease authorizes the particular physical range accessed. Confine raw pointers
and volatile memory operations to the narrow native adapter, with those lifetime,
range, and exclusivity obligations documented. No user virtual mapping grants a
privileged physical pointer merely because the numeric addresses coincide.

## Validation and disproof

Host tests must compare the populated hierarchy against an independent walk or
oracle and check exact permissions, distant/shared branches, guard gaps, unused
entries, frame purposes, and ownership/accounting. Inject failed allocations,
binds or physical operations where the API permits them. Exercise partial
population, corrupt entries, failed whole-arena clearing, and release failures;
verify that the owner retains all unreleased leases and cleanup succeeds on
retry without stale or double release. A callback failure must not silently
discard evidence or substitute an unchecked success value.

The native extension in
[`owned_mapping_test.rs`](../../kernel/src/arch/x86_64/owned_mapping_test.rs), called
by [`memory_test.rs`](../../kernel/src/arch/x86_64/memory_test.rs), exercises actual
table/data frames through the temporary alias, independent hierarchy inspection,
partial-construction rejection, complete cleanup, alias withdrawal, and restored
accounting. Controlled corruption of a ready tree is covered by host tests, not
claimed as a native injection. The transcript distinguishes the new inactive
construction from ADR 0012's existing A-to-B-to-A active-root proof; none of the
newly staged roots becomes active.

The added native records are `kernel.memory_test.physical_window.ready`,
`kernel.memory_test.staged.verified`, and `kernel.memory_test.staged.released`.
Four regions own seven data frames and nine table frames through five leases.
Five injected allocation-clear failures, one denied population write, and six
cleanup-clear failures each retain resources for successful retry and exact
restoration. A native partial clear zeros one word and returns failure; host
tests additionally exercise write-then-fail and every readback position in their
four-table fixture, totaling 2,048 positions.

Focused validation passed 81 x86 kernel tests, including eight owner and three
physical-access tests, plus eight xtask memory-transcript tests including three
new strict checks. Freestanding `memory_test` Clippy passed after precedence
warnings were corrected. Three independent reviews reported no actionable issue
after a cast-syntax correction. Initial native memory smoke passed at 96M and
320M, restoring all 24,158 and 81,502 free frames respectively. The complete
24-target regression matrix passed in 1,168.6 seconds of combined target time,
with all 233 frozen source/configuration files unchanged. Full kernel suites
passed 372 default and 415 all-feature tests; all 92 xtask tests passed.

Both TCG and KVM memory runs at 96M and 320M accepted exactly 17 ordered records
and restored the same free-frame counts, all leases, alias state, and bootstrap
root. Dispatcher and workspace regressions also passed on both backends. Normal,
dispatcher, and workspace images match their pre-change baselines; the initial,
TCG, and KVM memory proof images share SHA-256
`222b66ec621444552d3212443d081d007cf16b1b5ab6a350e8fb7de388423964`.
The [development log](../../notebook/Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md)
records final evidence links and preserved-image hashes. These results validate
inactive construction, not activation of those roots. No new VirtualBox run is
claimed; this increment changes no boot contract.

## Consequences and remaining gates

This proves a bounded path from inert geometry to owned, populated, inspected,
and reclaimable inactive tables. The temporary alias changes the feature proof's
active kernel root only for privileged access; it does not activate a staged user
root, move the kernel, or change Threshold's boot contract.

Per-user-leaf W^X and an NX physical alias do not eliminate the retained writable
and executable bootstrap identity alias. They therefore establish no global W^X,
KPTI, SMP/TLB shootdown, or production isolation claim. Before a low user root can
run, a later proof must keep kernel code/data, descriptor tables, interrupt/fault
entry, transition stacks, and physical/table access resident under every selected
root; define effective permission and alias rules; and prove activation, fault,
withdrawal, TLB, and recovery behavior.

General launch, multiple executable task bounds, user-copy, FPU/TLS, Reliquary,
and Linux execution remain separate gates in the
[readiness audit](../architecture/execution-readiness.md). This increment selects
no permanent high-half layout or foreign ABI and does not remove the live mapper's
low-branch restriction.

## Alternatives

- **Activate the new user root immediately:** rejected until all live kernel
  residency and transition requirements are independently established.
- **Drop a partially built owner on error:** rejected because allocator leases
  and failed clearing require explicit retained ownership and retry.
- **Treat a physical-window address as memory authority:** rejected; checked
  geometry does not establish an allocator claim or safe access.
- **Replace the kernel/boot layout in the same proof:** deferred to keep owned
  construction, temporary access, and live-root activation independently testable.
