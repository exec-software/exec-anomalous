# Owned low-address user execution

Date: 2026-09-27

## Scope

Proposed [ADR 0044](../../docs/adr/0044-prove-owned-low-address-user-execution.md)
adds a separate bounded ring-3 phase after the existing inactive construction
and resident CPL0 activation proofs. It keeps the earlier evidence distinct and
uses the existing mapping owner, explicit supervisor residency, physical-access
owner, and retained root session.

The new pure
[`user_entry_plan`](../../kernel/src/arch/x86_64/user_entry_plan.rs) checks
initialized executable-byte membership and guarded writable/nonexecutable stack
geometry. Initial RSP must be 16-byte aligned; resumed RSP must be 8-byte aligned
and remain inside the admitted stack. This is inert admission, not authority or
a serialized native launch format.

The native fixture in
[`user_mapping_test.rs`](../../kernel/src/arch/x86_64/user_mapping_test.rs) and
[`user_mapping_entry.S`](../../kernel/src/arch/x86_64/user_mapping_entry.S)
uses two roots, two real task identities, and two real circles. Each root owns
one RX code page at 4 MiB, two RW+NX data pages at 8 MiB, and two guarded RW+NX
stack pages near the lower canonical user limit. Each circle retains three
mapping children. The complete fixture owns ten data frames, eighteen table
frames, and eight active/bound leases.

## Implemented native path

The sequence is `bootstrap -> B -> A -> bootstrap`. B first executes two exact
UD2 reports and resumes at two different noninitial user stack pointers, the
first inside a nested call. An exact user read
of supervisor kernel text at 1 MiB then faults B with page-fault error 5. A must
still execute, emit its own two resumed reports, and reach its exact private
UD2 exit. Equal virtual addresses must retain distinct private data across the
two roots.

The assembly preserves the Rust continuation, clears user-visible GPRs, keeps
IF clear, and brackets the GPR-only fixture with explicit CR0.TS restoration.
The existing separate resident privilege stack supplies TSS.RSP0 and is wiped
after each terminal user return. The proof preserves and checks the prior TSS
selection, clears saved continuations, and retains the preceding BSP/NMI/CPU-control restrictions.
Report and terminal acceptance requires exact proof phase, task/circle/root,
vector/error, instruction, selectors, flags, and full saved stack state. An
unexpected exception remains fatal with the owners retained.

The shared exception snapshot now reads the actual long-mode hardware-saved RSP
and SS for ordinary CPL0, CPL3, and IST frames. Its complete saved frame is 176
bytes. Host fixtures cover all three origins; the earlier resident probes
independently used those hardware words already. Because this corrects shared diagnostic implementation,
normal, dispatcher, and workspace images changed. The final evidence below
reports measured hashes and direct feature exclusion rather than byte identity
with their preceding images.

After terminal fault and exit, bootstrap restoration and root unregistration
must precede owned mapping cleanup. Detach mapping/address-space children before destroying task/circle identities,
confirm no attached contexts remain, prove generation retirement, remove
the physical alias, and restore allocator accounting exactly. No failed cleanup
may silently discard a retained owner.

## Transcript contract

The [`xtask` verifier](../../tools/xtask/src/main.rs) adds `user.ready`,
`user.verified`, and `user.released` immediately after `resident.released` and
before the unchanged `passed` record. The complete native transcript has
**23 structured records** and **20 selected lifecycle records**. Old records
retain their meanings and schemas.

The exact new fields are defined in ADR 0044 and enforced by the verifier:
readiness accounts for roots/tasks/circles, regions, frames, leases, code base,
and entry/stack/profile admission; verification requires two entries, four
resumed reports, three root switches, one faulted and one exited task, and every
isolation/restoration predicate; release requires zero remaining resources and
boot-equal free frames.

The existing three owned-mapping mutation groups now include all new fields,
covering scalar value/type changes, missing/duplicate fields, lifecycle loss,
duplication/reordering, unknown event names, and extra private content. The
normal-image exclusion test covers the new event names. These additions extend
existing test groups rather than adding test cases to the suite count.

## Completed validation

The initial TCG `make memory-smoke` run passed at **96 MiB and 320 MiB**. Both
runs produced all **23 structured records** and passed the exact four-report,
supervisor-fault, peer-progress, exit, and full resource/state restoration
contract. Free frames returned to **24,150** and **81,494** respectively. The
initial native transcript (`initial-memory-smoke.log`; not distributed)
is retained outside the repository.

The full **24-target regression matrix passed**, totaling **1,346.9 seconds**
of combined target time and finalized at `2026-09-27T16:36:10+00:00`. This includes
normal-image exclusion and memory, dispatcher, and workspace runs under both
TCG and KVM. All **240 frozen source/configuration files** remained unchanged.
Focused checks passed **107 x86 kernel tests**, **eight memory-transcript tests**,
and freestanding Clippy. Full suites passed **398 default kernel tests**,
**441 all-feature kernel tests**, and **92 xtask tests**.

Both TCG and KVM memory proofs passed at **96 MiB and 320 MiB**. Each run
produced 23 structured records and verified four resumed user reports, one
contained supervisor-read fault, one peer exit, and exact reclamation of the
new phase's 28 frames and eight leases. Both backends restored 24,150/81,494
free frames and all transition/task/circle state. Initial, TCG, and KVM memory
images match each other.

Both dispatcher runs retained 64 preemptions, 80 PIT ticks, 66 dispatches, one
contained fault, one exit, 16 survivor ticks, intact canaries, and complete
restoration. Both workspace runs retained 21 keyboard, 410 pointer/Appearance,
and 23 graphics frame checks, resource restoration, and shell fallback.

Normal, dispatcher, and workspace images **changed** with the shared exception
snapshot correction. Their behavior passed the full native regressions and
normal-image feature exclusion; no byte-identity claim is made for them.

| Image or proof copies | Final SHA-256 |
| --- | --- |
| `anomalous.img` | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| `dispatcher-test.img` | `675c433b2ea97bd2a73552ace7fc9a413bb05ddc98654d87ba806b07300b39aa` |
| `workspace-dev.img` | `65d6cfe39038ac6c078139e9d052a3f790f31eb8b1a018ebd3243cf904251077` |
| Initial/TCG/KVM memory proof | `71e81fa8385118c56fde3d539b4f7e13179dd19afcf67040a4585068f266a641` |

The local validation report (`user-mapping-validation.md`; not distributed)
links every target log and timing. The
machine-readable summary (`summary.json`; not distributed)
records outcomes, native predicates, and evidence hashes, while the
source manifest (`source-sha256.json`; not distributed)
records the unchanged validated source set. These are development-session
artifacts outside the repository. No new VirtualBox run is claimed; the earlier
`VERR_SVM_IN_USE` limitation remains separate from the measured TCG/KVM result.
ADR 0044 remains Proposed.

## Remaining limits

The reports and exit are a feature-local handshake, not a syscall ABI. The task
profile is GPR-only and runs without asynchronous scheduling. It adds no ELF or
Mach-O parser, native artifact encoding, TLS, FPU/SIMD ownership, general
user-copy policy, or permanent kernel layout. The privileged alias still permits
writes to executable physical storage, so this does not establish global W^X.
Existing desktop and scheduler profiles and the legacy mapper's retention gap
remain separate work.

The next bounded slice is a retained prepared-launch owner for this same
three-region GPR-only profile. It should cover mapping build/population and real
task/circle attachment, an explicit commit, and retryable cancellation of partial
preparation or an unstarted ready task. Inject each preparation failure and
require no runnable task leaks and exact restoration, then enter and exit the
same successful fixture. Keep input inert and format-neutral and supply
authority separately; serialized ABI and ELF parsing remain later work.
