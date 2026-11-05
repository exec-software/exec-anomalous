# Inactive owned mapping construction

Date: 2026-09-26

## Scope

The next authorized memory slice joins the inert geometry from
[ADR 0041](../../docs/adr/0041-plan-bounded-user-mappings-and-physical-access.md)
to the actual allocator ownership rules from
[ADR 0012](../../docs/adr/0012-prove-bounded-bootstrap-memory-ownership.md).
Proposed [ADR 0042](../../docs/adr/0042-prove-inactive-owned-user-mapping-construction.md)
records construction and reclamation of a complete bounded user hierarchy while
leaving its root inactive.

The implemented `staged_mapping` in
[`owned_mapping.rs`](../../kernel/src/arch/x86_64/owned_mapping.rs) consumes an inert mapping plan,
owns an exact contiguous table arena and per-region data leases, and verifies
populated pages against the requested geometry and permissions. Every successful
allocation is retained before the next fallible operation. The caller keeps the
mutable owner on failure rather than losing its partially acquired resources.
Its unsafe `build`, `validate`, and `cleanup` methods use the explicit unsafe
`mapping_memory` backend contract. Safe root/data/count/readiness getters expose
metadata only. Parent entries are populated after children, then all 512 entries
per table are read back against the exact expected values; unplanned entries and
even accessed/dirty additions fail validation because these roots never ran.

Cleanup first clears and verifies the entire table arena. A failure keeps
unreleased handles quarantined and supports retry. Only successful withdrawal
permits unbinding/releasing the data and tables; later release failures also
retain their handles and progress. Clearing failures must never be treated as
successful erasure or restored accounting.
Active-root/foreign-allocator preflight rejection preserves the prior owner and
readiness. Mutating cleanup failure retains a withdrawing, unready owner until
retry succeeds. There is no automatic fallible `Drop` cleanup.

## Native access boundary

The new phase in
[`owned_mapping_test.rs`](../../kernel/src/arch/x86_64/owned_mapping_test.rs), invoked
by [`memory_test.rs`](../../kernel/src/arch/x86_64/memory_test.rs), retains the selected kernel root and
temporarily borrow its validated supervisor bootstrap `[0, 4 GiB)` hierarchy into
an empty high-half slot. The test candidate is `0xffff_9000_0000_0000`, PML4 slot
288, with effective supervisor RW+NX permissions. This preserves the low resident
kernel paths and chooses no permanent direct-map or high-half kernel layout.
Validate the exact six-page Threshold hierarchy before alias installation;
change only the added root entry, keeping shared descendants executable for
their existing low path. The bridge rejects IF enabled and CR4 PGE/PCIDE/LA57,
and reloads CR3 immediately after its root-entry changes for local invalidation.
This is a serialized bootstrap-processor proof, not SMP or PCID handling.
The retained alias owner and volatile backend are in
[`physical_access.rs`](../../kernel/src/arch/x86_64/physical_access.rs).

The alias's arithmetic cannot establish physical availability or ownership.
Allocator callbacks and validated leases authorize exact physical ranges; the
native adapter alone performs raw pointer/volatile accesses. Borrowed bootstrap
tables must not be released as owned frames. After all stage owners clean up,
withdraw the alias, flush translations, and verify restoration of the root/slot.

The staged user roots are never selected in CR3. The retained writable/executable
bootstrap identity alias still prevents a global W^X claim. Existing active-root
memory tests remain separate evidence and must not be presented as execution of
these newly constructed low user mappings.

## Validation — passed

Focused x86 kernel tests passed **81 tests**, including eight new owner tests and
three physical-access tests. The host tests exercise independent walks, exact
permissions, guards/unused entries, every populated-entry write failure,
allocation clear/exhaustion boundaries, each cleanup-clear failure, and every
readback position in a four-table fixture: **2,048 positions**. Host write faults
may perform the write and then report failure. Unexpected entries, altered
targets, and permission corruption withdraw readiness on validation failure.

The eight selected xtask memory-transcript tests passed, including three new
strict checks. Freestanding `memory_test` Clippy passed after initial precedence
warnings were fixed. Three independent reviews found no actionable issue after
the cast-syntax correction. The focused logs are `focused-kernel.log`,
`focused-xtask.log`, and `focused-clippy.log` under
an external validation directory (not distributed).

Initial native memory smoke passed at **96M and 320M**, restoring exactly
**24,158 and 81,502 free frames**, respectively. The added records are
`kernel.memory_test.physical_window.ready`, `kernel.memory_test.staged.verified`,
and `kernel.memory_test.staged.released`. Each run verifies four regions, seven
data frames, nine table frames, and five bound leases before cleanup. It injects
five allocation-clear failures, one denied population write, and six cleanup-clear
failures; retries restore all resources. Native partial clearing zeros the first
word and returns false. Native population failure denies a write; ready-tree
corruption and write-then-fail evidence remain host tests. The retained original
A-to-B-to-A proof does not activate the newly staged roots.

The full **24-target regression matrix passed**, totaling **1,168.6 seconds**
of combined target time and finalized at `2026-09-27T01:12:05+00:00`. The full
kernel suites passed **372 default** and **415 all-feature** tests; all **92
xtask tests** passed. Finalization verified that all **233 frozen
source/configuration files** remained unchanged. All targets returned zero:

- `check`, `smoke`, `fault-smoke`, `circle-smoke`, `control-smoke`, `rite-smoke`;
- `scheduler-smoke`, `dispatcher-smoke`, `supervision-smoke`,
  `shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`, `input-wait-smoke`,
  `shell-smoke`;
- `display-vessel-smoke`, `supervised-renderer-smoke`,
  `semantic-surface-transport-smoke`, `semantic-presentation-smoke`,
  `desktop-smoke`, `workspace-smoke`, and `normal-exclusion`;
- additional `memory-smoke`, `dispatcher-smoke`, and `workspace-smoke` with KVM.

Both TCG and KVM memory runs passed at **96M and 320M**, with exactly **17
ordered records** per run. Each restored the same 24,158/81,502 free-frame counts,
released every lease, removed the temporary alias, and retained the bootstrap
root. Both backends also retained the dispatcher proof's 64 preemptions, 80 timer
ticks, 66 dispatches, one contained fault, 16 survivor ticks, one exit, intact
canaries, and restored resources. Both workspace runs retained 21 keyboard,
410 pointer/Appearance, and 23 graphics frame checks, resource restoration,
and shell fallback.

Normal, dispatcher, and workspace images are byte-identical to their pre-change
baselines. The initial, TCG, and KVM copies of the new memory proof image also
match each other:

| Image or proof copies | Final SHA-256 |
| --- | --- |
| `anomalous.img` | `bb36c1cf49de28a594f83a5ea3b81b2ba99cd9b5c1c6997953f2dc0e45957e9f` |
| `dispatcher-test.img` | `bf7a36b68bd696c3b15898350b82421793ec72458098240540f0ead9a53f7e6c` |
| `workspace-dev.img` | `7ef45ff45e6592ee1399f8922e95a87dc946afcbdb19004464a445e2d1275e9d` |
| Initial/TCG/KVM memory proof | `222b66ec621444552d3212443d081d007cf16b1b5ab6a350e8fb7de388423964` |

The local validation report (`owned-mapping-validation.md`; not distributed)
links every target log, timing, and saved memory image. The
machine-readable summary (`summary.json`; not distributed)
records outcomes, native predicates, and evidence hashes; the
source manifest (`source-sha256.json`; not distributed)
records the unchanged validated source set. These are development-session
artifacts outside the repository. No new VirtualBox run is claimed; this
increment changes no boot contract. The new user roots remained inactive
throughout these proofs.

## Remaining migration work

Source audit also identified an existing ownership-recovery gap in the legacy
[`address_space::create`](../../kernel/src/arch/x86_64/address_space.rs) rollback
path: if releasing its local table lease fails, it returns `rollback_failed`
and drops that handle. The allocator keeps the frames quarantined, but the last
reachable owner is lost. The new staged owner retains its handles across such
failures; this increment did not change or fix the legacy live mapper. Migrating
that path remains separate work.

The later activation gate must address kernel code/data and descriptor residency,
interrupt/fault entry, transition stacks, privileged physical access under each
root, aliases/effective W^X, TLB behavior, and safe recovery. This work does not
establish a permanent high-half kernel, remove the live low-branch restriction,
freeze an ABI, or run Linux. The
[execution-readiness audit](../../docs/architecture/execution-readiness.md)
continues to track those independent prerequisites.
