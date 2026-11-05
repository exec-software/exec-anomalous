# Resident mapping activation

Date: 2026-09-26

## Scope

Proposed [ADR 0043](../../docs/adr/0043-prove-resident-mapping-activation.md)
extends the previous inactive construction proof with a separate bounded CPL0
root-selection experiment. The earlier inactive proof remains distinct; this
increment adds explicit supervisor residency and transition ownership rather
than treating its user-only roots as runnable.

The pure [`resident_plan`](../../kernel/src/arch/x86_64/resident_plan.rs) permits
at most eight supervisor identity ranges within `[0, 2 MiB)`, excluding user
mapping collisions and W+X. The actual profile uses linker-defined text RX,
rodata R+NX, data/BSS RW+NX, and Threshold's full 64-KiB stack RW+NX. All other
pages there are absent. One additional owned leaf table per root and the borrowed
supervisor RW+NX alias at slot 288 provide the bounded mixed hierarchy.

[`owned_mapping.rs`](../../kernel/src/arch/x86_64/owned_mapping.rs) retains the
existing construction and cleanup ownership, adds mixed profiles, and explicitly
prepares activation before allowing tightly scoped hardware accessed/dirty bits.
That preparation changes validation policy, not CR3 or execution state.

The private `root_session` in
[`active_mapping_test.rs`](../../kernel/src/arch/x86_64/active_mapping_test.rs)
holds mutable borrows of two owners and the
[`physical-access owner`](../../kernel/src/arch/x86_64/physical_access.rs).
Admission records every successful registration. Selection validates the retained
root/stack/alias; finish restores bootstrap before unregistering. Owners cannot
be cleaned through the ordinary proof flow until finish succeeds. Unexpected
failure halts with retained state; there is no automatic fallible destruction
or general recovery service.

## Native evidence

Two mixed roots each retain four user regions, seven data frames, and ten table
frames. One separate physical collision probe at `0x0040_0000` adds an unbound
data lease: 14 mapped data frames, 20 table frames, one probe frame, eleven active
leases, and ten bound leases in total.

The sequence `bootstrap -> A -> B -> A -> bootstrap` changes CR3 four times.
Root-local values and writes must remain isolated. Low user virtual address
`0x0040_0000` names each root's private code/data, while the high alias must
continue to reach the independently owned physical frame with that number.
Each root performs one controlled read-only write, guard read, and NX execution
fault. Exact phase/root/error/address/instruction/stack predicates govern the
single-use resume; this is CPL0 evidence, not ring-3 execution.

Before activation, verify live code, descriptor and exception-stack residency,
the complete supervisor stack, and actual permissions. Keep IF clear/NMI masked
on the BSP, require WP/NXE, and exclude PGE/LA57/PCIDE/SMEP/SMAP/PKE/CET/PKS from
the proof's CPU state. After restoring bootstrap, revalidate only permitted A/D
changes, clean both owners, release the probe, and remove the alias.

## Transcript and completed validation

The [`xtask` memory verifier](../../tools/xtask/src/main.rs) now expects
`resident.ready`, `resident.verified`, and `resident.released` after
`staged.released` and before the unchanged `passed` record. This yields twenty
native structured records and seventeen strictly ordered selected lifecycle
records. The existing parser treatment of unrelated boot records and Threshold
text remains unchanged. New event keys, scalar types, counts, booleans, canonical
hex values, and free-frame equality are exact.

The existing three staged-event mutation tests now also cover resident events,
including every field's value/type mutation, removal, and duplication; event
loss, duplication/reordering, unknown names, and extra private fields; and normal
image event exclusion.

Focused checks passed: 94 x86-64 kernel tests with 291 filtered, eight memory
transcript tests with 84 filtered, and the freestanding Clippy check. Thirteen
kernel tests were added: five resident-plan tests, four mixed-owner tests, three
fault-probe tests, and one descriptor-residency test. The transcript mutation
coverage extends existing tests rather than adding test cases to that total.

The full **24-target regression matrix passed**, totaling **1,111.9 seconds** of
combined target time and finalized at `2026-09-27T03:17:35+00:00`. The full kernel
suites passed **385 default** and **428 all-feature** tests; all **92 xtask tests**
passed. Finalization verified all **236 frozen source/configuration files**
remained unchanged. This includes normal-image exclusion and the additional
memory, dispatcher, and workspace runs under KVM.

Both TCG and KVM memory proofs passed at **96M and 320M**. Every run produced
twenty structured records, observed six exact page faults, reclaimed all 35 owned
frames and eleven leases, and restored the alias/root state and initial free
counts of 24,152 and 81,496. Both dispatcher runs retained 64 preemptions, 80 timer
ticks, 66 dispatches, one contained fault, one exit, 16 survivor ticks, intact
canaries, and restored resources. Both workspace runs retained 21 keyboard,
410 pointer/Appearance, and 23 graphics frame checks, resource restoration,
and shell fallback.

Normal, dispatcher, and workspace images are byte-identical to their baselines.
The initial, TCG, and KVM memory proof images also match each other:

| Image or proof copies | Final SHA-256 |
| --- | --- |
| `anomalous.img` | `bb36c1cf49de28a594f83a5ea3b81b2ba99cd9b5c1c6997953f2dc0e45957e9f` |
| `dispatcher-test.img` | `bf7a36b68bd696c3b15898350b82421793ec72458098240540f0ead9a53f7e6c` |
| `workspace-dev.img` | `7ef45ff45e6592ee1399f8922e95a87dc946afcbdb19004464a445e2d1275e9d` |
| Initial/TCG/KVM memory proof | `f12ed211703fec8034caf79449b6d58f45015f0f6c0c40821bb65a51bf261ec3` |

The first native attempt rejected `resident_linker_geometry` before any
resident-root CR3 change. Comparisons using separate `extern static u8` linker
symbols had been folded to false: `nm` showed valid adjacent symbol addresses,
while `objdump` showed the unconditional admission failure. Reading those linker
addresses numerically with `lea` and `sym` operands removed that object-address
assumption; the repeated TCG proof passed. This was an admission-check defect,
not a failure after selecting a resident root.

Evidence lives under
an external validation directory (not distributed):
`focused-kernel.log`, `focused-xtask.log`, `focused-clippy.log`,
`initial-memory-smoke.log`, and the retained `linker-admission-failure.log`.
The local validation report (`resident-mapping-validation.md`; not distributed)
links every target log, timing, and saved memory image. The
machine-readable summary (`summary.json`; not distributed)
records outcomes, native predicates, and evidence hashes; the
source manifest (`source-sha256.json`; not distributed)
records the unchanged validated source set. These are development-session
artifacts outside the repository. No new VirtualBox run is claimed; the earlier
host-backend limitation, `VERR_SVM_IN_USE`, remains separate from the completed
TCG/KVM proof.

## Remaining limits

Kernel identity pages enforce their selected permissions, but the high physical
alias remains writable even for underlying text. The restored bootstrap map is
also still writable/executable. This is not a global W^X or permanent high-half
kernel result. General application launch, ring-3 fault handling, concurrency,
safe user-copy, and Linux execution remain later gates. This increment adds no
ABI or authentication mechanism.

Review also identified a shared diagnostic migration gate. The resident probe
bounds the complete 176-byte exception frame and reads saved RSP/SS directly from
words 20/21, while unchanged
[`exceptions::snapshot_from_frame`](../../kernel/src/arch/x86_64/exceptions.rs)
still synthesizes RSP and omits SS for same-CPL, non-IST frames. None of the six
controlled probes relies on those diagnostic stack fields. Normalize the shared
frame contract and add CPL0/CPL3/IST fixtures before generic returning-fault or
user-copy recovery; this increment does not claim that migration is complete.
