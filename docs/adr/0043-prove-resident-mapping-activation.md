# ADR 0043: Prove bounded resident mapping activation

- Status: Proposed
- Date: 2026-09-26

## Context

[ADR 0042](0042-prove-inactive-owned-user-mapping-construction.md) proves owned
user-tree construction, privileged physical access, retained failure ownership,
and cleanup without selecting the new roots. Those user-only roots contain no
kernel residency and cannot support a root transition. The next experiment must
make the executing kernel paths explicit before changing CR3, while retaining
the independent physical access that prevents low user addresses from redirecting
privileged accesses.

This record adds a separate bounded activation phase to `memory_test`. It does
not reinterpret the preceding inactive proof as an activation result, generalize
the legacy mapper, or introduce a process loader or stable native ABI.

## Proposed decision

### Describe a bounded supervisor region explicitly

Add the pure checked
[`resident_plan`](../../kernel/src/arch/x86_64/resident_plan.rs) beside the user
mapping plan. It accepts at most eight disjoint supervisor identity ranges within
`[0, 2 MiB)`, forbids W+X and user permission on those ranges, excludes user
mappings from that complete low region, and requires the existing low PDPT/PD
prefixes. One additional owned leaf table maps the supervisor pages; the combined
table budget remains checked against the existing capacity. The fixed borrowed
high alias is supervisor RW+NX in PML4 slot 288 at `0xffff_9000_0000_0000`.
These descriptions establish geometry, not the lifetime or authority of the
borrowed resident pages and alias tables.

The native profile identity-maps only the following 4-KiB pages:

- kernel text as supervisor RX;
- kernel rodata as supervisor R+NX;
- kernel data and BSS as supervisor RW+NX; and
- Threshold's complete 64-KiB bootstrap stack as supervisor RW+NX.

All other pages in the low supervisor region remain absent. Validate aligned
linker section bounds, code residency, the current stack with transition
headroom, and the live descriptor/exception-stack state before any selection.
The proof rejects a kernel that does not fit its bounded region. Page-aligned
linker symbols describe this experiment's sections without relocating the kernel
or selecting a permanent high-half layout.

### Prepare and retain mixed owners

Extend [`staged_mapping`](../../kernel/src/arch/x86_64/owned_mapping.rs) with an
optional resident profile. User-only construction keeps the preceding contract.
The mixed owner retains its exact contiguous table arena, per-region data leases,
and borrowed-residency metadata. Population and validation cover the complete
expected user and supervisor hierarchy plus the exact borrowed alias entry.

An explicit unsafe `prepare_activation` first validates a ready inactive mixed
owner, then enables narrowly defined hardware-bit tolerance. It does not select
the root or prove that it has run. Present non-leaf entries may gain accessed;
present read-only/RX leaves may gain accessed; writable leaves may gain accessed
and dirty. Absent entries remain exactly zero, and targets, permissions, and all
other bits remain exact. Successful full cleanup resets this validation mode.

The private `root_session` in
[`active_mapping_test.rs`](../../kernel/src/arch/x86_64/active_mapping_test.rs)
borrows two distinct mutable mapping owners and the physical-access owner for
the entire transition. `begin` prepares and registers both roots, recording each
successful registration immediately. `select` validates the retained state,
changes CR3, and verifies the selected root and alias. `finish` restores the
bootstrap root and flushes translations before unregistering either borrower.
Cleanup is permitted only after successful finish and the end of those borrows.

These are private unsafe feature-proof operations, not a general safe runtime
API. An error retains the session's owners and registration state. There is no
fallible automatic `Drop` cleanup; an unexpected native failure reports its
static reason and halts with the ownership still retained. The experiment does
not promise general exception recovery or automatic retry of failed transitions.

[`bootstrap_physical_access`](../../kernel/src/arch/x86_64/physical_access.rs)
admits only the two explicitly registered mixed roots in addition to its original
bootstrap root. Every physical access validates the current root and retained
alias. Removal requires the bootstrap root selected and no registered borrowers.
Keep IF clear, NMI masked, and the proof on the BSP. Require WP and NXE and reject
CR4 PGE, LA57, PCIDE, SMEP, SMAP, PKE, CET, and PKS so unrelated translation or
protection modes cannot account for the intended fault evidence. Root changes
flush local translations; this defines no SMP or PCID invalidation protocol.

### Exercise root selection and exact faults

Construct two roots with the prior four-region user fixture. Each owns seven
data pages and ten table pages, including its extra supervisor leaf table. A
separate unbound one-frame lease at physical `0x0040_0000` holds a collision
marker. The totals are fourteen mapped data frames, twenty table frames, one
probe frame, eleven active leases, and ten bound leases.

Select `bootstrap -> A -> B -> A -> bootstrap`: four CR3 changes. Check distinct
user values, writable-state persistence, and absence of cross-root corruption.
Both roots map user virtual `0x0040_0000` to their own different data frames;
reads through the high physical alias must still reach the independently owned
physical frame at the numerically equal address. This is the concrete physical
pointer collision that inert planning alone could not exercise.

For each mixed root, arm exactly one read-only write fault, one absent guard
read fault, and one NX execution fault. The
[`single-use probe handler`](../../kernel/src/arch/x86_64/resident_probe.rs)
must match the active
proof phase, root, vector/error, fault address, instruction/resume site, and
resident stack before a single-use resume is allowed. Unexpected faults retain
the existing fatal behavior. A missing NX fault must fail the probe rather than
execute arbitrary bytes. These are CPL0 probes of the selected translations;
they do not demonstrate ring-3 process execution or general application fault
handling.

After restoring bootstrap, validate both owners with only the admitted A/D
differences, then clean them up, release the collision lease, and remove the
temporary alias. All allocator accounting must return to its initial values.
The borrowed bootstrap and resident storage is never released as owned memory.

## Validation and evidence contract

Preserve the preceding memory-proof records and add exactly three before the
existing `kernel.memory_test.passed` record:

| Event suffix under `kernel.memory_test` | Required evidence |
| --- | --- |
| `resident.ready` | Two roots; four user regions per root; 14 data, 20 table, and one probe frame; 11 active/10 bound leases; checked kernel permissions, descriptors, and supervisor stack. |
| `resident.verified` | Four switches; low code and physical collision both `0x0000000000400000`; isolated values and preserved physical alias; two faults of each kind; exact resume, constrained A/D validation, and restored bootstrap. |
| `resident.released` | Initial/free counts equal boot evidence; zero owned frames and leases; alias removed and kernel root restored. |

The native transcript contains twenty structured records. The
[`xtask` verifier](../../tools/xtask/src/main.rs) preserves its existing unrelated
boot/text handling and requires the seventeen selected lifecycle events in exact
order. Every selected event has exact keys, scalar types, and required values.
Mutation tests alter values/types, remove or duplicate every field, remove or
duplicate/reorder events, add private fields, and introduce unknown memory
events. QEMU's success status remains independently required. The final legacy
`passed` fields continue to describe its original two-root/three-switch proof;
the separate resident event describes this phase's four switches.

Host tests must cover resident-profile collision/capacity/permission rejection,
mixed-tree population, borrowed alias constraints, exact A/D allowances, and
unchanged failure/cleanup ownership. Native tests must exercise both RAM sizes,
TCG and KVM, the exact fault and collision predicates, and complete restoration.
Run the full repository regression matrix and normal-image exclusion checks.

Initial focused validation passed: 94 x86-64 kernel tests (291 filtered), including
13 added tests across resident planning, mixed ownership, fault probes, and
descriptor residency; eight memory-transcript tests (84 filtered); and the
freestanding Clippy check. The three existing staged mutation groups now also
exercise the resident records.

The full 24-target matrix passed in 1,111.9 seconds of combined target time,
including normal-image exclusion and memory, dispatcher, and workspace proofs
under both TCG and KVM. Full host suites passed 385 default kernel tests, 428
all-feature kernel tests, and 92 xtask tests. Finalization verified all 236 frozen
source/configuration files remained unchanged.

Both native backends passed at 96M and 320M with twenty structured records each.
Each run observed all six exact page faults, reclaimed all 35 owned frames and
eleven leases, removed the temporary alias, and restored the bootstrap root.
Free frames returned exactly to 24,152 and 81,496 respectively. Normal,
dispatcher, and workspace images remained byte-identical to their baselines.
The initial, TCG, and KVM memory proof images all have SHA-256
`f12ed211703fec8034caf79449b6d58f45015f0f6c0c40821bb65a51bf261ec3`.

The local validation report (`resident-mapping-validation.md`; not distributed)
and machine-readable summary (`summary.json`; not distributed)
retain individual target logs, timings, native predicates, and artifact hashes.
These are development-session artifacts outside the repository. No new
VirtualBox run is claimed; the earlier `VERR_SVM_IN_USE` host-backend limitation
remains distinct from this TCG/KVM evidence.

## Consequences and remaining limits

The proof makes selected low user mappings active while explicitly retaining
the supervisor paths required by this kernel experiment. It does not provide a
permanent kernel layout, a production activation service, ring-3 launch,
asynchronous scheduling, SMP, general user-copy, or Linux/Mach-O execution.

Supervisor text has RX permissions in the identity mapping, but the borrowed
high alias still gives CPL0 writable access to the same physical text, and the
restored bootstrap identity map remains writable/executable. This is not global
W^X or production isolation. Durable privileged residency, alias hardening,
real application transitions, and the remaining
[execution gates](../architecture/execution-readiness.md) need separate proofs.
No identity/authentication, token format, or native ABI is added here.

The resident probe bounds the complete 176-byte exception frame and directly
reads the hardware-saved RSP and SS at words 20 and 21. The shared
[`exceptions::snapshot_from_frame`](../../kernel/src/arch/x86_64/exceptions.rs)
diagnostic remains unchanged: same-CPL, non-IST snapshots still synthesize RSP
and omit SS. The six probes do not rely on those diagnostic stack fields.
Normalize that shared contract, with CPL0, CPL3, and IST fixtures, before adding
generic returning-fault or user-copy recovery.

## Alternatives

- **Select the prior user-only roots:** rejected because they lack the executing
  kernel, stack, descriptor, and fault-path mappings.
- **Borrow the entire low bootstrap branch:** rejected for this proof because it
  conflicts with low user mappings and retains broad supervisor permissions.
- **Ignore arbitrary entry differences after hardware use:** rejected; only the
  precisely permitted accessed/dirty additions may differ.
- **Use an unretained CR3 helper:** rejected because owner, alias, and restoration
  lifetimes must remain explicit across failure and cleanup.
- **Move immediately to a permanent high-half kernel or general process launch:**
  deferred so residency, root transitions, and fault evidence remain bounded.
