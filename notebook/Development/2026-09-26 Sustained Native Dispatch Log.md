# Sustained native dispatch and execution readiness

Date: 2026-09-26

## Scope

The user selected remaining kernel foundations before a modular native execution
boundary, Linux ELF first, and macOS afterward. The existing desktop and native
window-content work remain available; this increment does not add another
desktop feature or filesystem application.

**Observed by source audit:** the native format is not finished. Experimental
rite call/reply prefixes and operation-specific payloads exist, but semantic
invocation canonical encoding remains open. Reliquary/Covenant remain proposed
and unimplemented. The only ELF parsing is host build inspection of fixed linked
artifacts. There is no runtime Linux loader or Linux syscall personality.
The durable [execution-readiness audit](../../docs/architecture/execution-readiness.md)
records source anchors, limits, and the ordered follow-up gates.

| Layer | State at the start of this increment |
| --- | --- |
| Native rites | Implemented experimental fixed-width boundary; not stable ABI. |
| Semantic invocation encoding | Header exists; canonical serialization and negotiation remain open. |
| Native launch/artifact format | Hardcoded launch profiles; no versioned general launch descriptor or Reliquary encoding. |
| Kernel execution | Fixed private-task, scheduling, supervision, and known-fault proofs; general application prerequisites remain open. |
| Linux | Intended first foreign executable target; no runtime ELF or syscall personality. |
| macOS | Intended subsequent independent Mach-O/Darwin target; not based on Linux. |

## Selected experiment

**Implemented under proposed**
[ADR 0039](../../docs/adr/0039-prove-bounded-sustained-native-dispatch.md): a
separate removable `dispatcher_test` and `make dispatcher-smoke`. Two
private GPR-only tasks undergo 64 real PIT switches. One then executes its exact
known `ud2`; the survivor demonstrates continued user progress through 16
further timer ticks without another runnable peer. The no-peer path returns to
the current validated frame and does not manufacture dispatch-sequence advances.

The terminal path must restore kernel CR3, TSS RSP0, CR0.TS, and the saved
kernel continuation before full verified withdrawal and clearing. The strict
transcript must distinguish actual switches from timer ticks, reject altered
counts/order/progress/fault attribution, and prove initial frame-accounting
restoration. Prior scheduler and desktop behavior are retained.

This is a bounded mechanism proof. It does not claim FPU/SIMD or TLS switching,
general user-copy or user faults, dynamic executable admission, SMP, production
scheduling, or Linux compatibility. The owned mapper still has a reserved low
kernel branch, three mapping extents, and one user leaf-table branch. These
constraints need explicit follow-up work before a broader launch contract.

## Validation

Final regression runs passed under both QEMU TCG and KVM. Each accepted
exactly 14 ordered records: eight boot records followed by six aggregate
`kernel.dispatcher_test` records. Both restored all **65,112** initial free
frames and reported verified storage clearing and kernel-continuation restoration.

| Measured predicate | TCG | KVM |
| --- | ---: | ---: |
| Private tasks | 2 | 2 |
| Timer-driven task switches | 64 | 64 |
| Quanta per task during the two-task phase | 32 | 32 |
| Sole-survivor timer ticks | 16 | 16 |
| Total PIT ticks | 80 | 80 |
| Actual dispatches | 66 | 66 |
| Contained exact faults / successful exits | 1 / 1 | 1 / 1 |
| Task and survivor progress, canaries intact | true | true |
| Initial and final free frames | 65,112 | 65,112 |

The initial dispatch, 64 switches, and one dispatch after the fault account for
all 66 dispatches. Returning to the current survivor for 16 further timer ticks
does not advance dispatch or preemption sequence. The two task spaces own 14
data frames and eight page-table frames. This finite run is not a fairness,
latency, or real-time guarantee.

The final dispatcher image SHA-256 is
`bf7a36b68bd696c3b15898350b82421793ec72458098240540f0ead9a53f7e6c`.
The development-session evidence directory is
an external validation directory (not distributed);
`dispatcher-smoke.log`, `dispatcher-smoke-kvm.log`, `results.tsv`, and
`summary.json` retain the final results and identities. The earlier
`final-focused-tcg.log` and `final-focused-kvm.log` remain focused-run evidence
for the same image hash. These are local generated evidence, not repository
sources.

The six new `xtask` tests passed, covering exact transcript acceptance, line
endings, missing/duplicate/reordered records, incorrect counts and failed
invariants, missing/extra/private fields, process-status correlation, and
feature removal/selection. All **89 `xtask` tests** passed in the final
`make check`, together with the host suites, formatting, script checks, and
warning-denied host/freestanding Clippy, including `dispatcher_test`,
`dispatcher_test,workspace_dev`, and the all-feature profiles.

The full **23-target** matrix passed in **1,071.6 seconds (17 minutes 51.6
seconds)**: 21 ordinary targets plus separate KVM dispatcher and workspace
repeats. All **228 frozen source/configuration files** were unchanged during
that run. ADR 0039 remains Proposed.

| Target | Result | Seconds |
| --- | --- | ---: |
| `check` | passed | 255.2 |
| `smoke` | passed | 1.5 |
| `fault-smoke` | passed | 1.3 |
| `circle-smoke` | passed | 34.6 |
| `control-smoke` | passed | 35.3 |
| `rite-smoke` | passed | 32.8 |
| `scheduler-smoke` | passed | 32.7 |
| `dispatcher-smoke` | passed | 32.3 |
| `supervision-smoke` | passed | 33.4 |
| `shell-vessel-smoke` | passed | 35.6 |
| `memory-smoke` | passed | 34.1 |
| `interrupt-smoke` | passed | 32.4 |
| `input-wait-smoke` | passed | 34.0 |
| `shell-smoke` | passed | 0.6 |
| `display-vessel-smoke` | passed | 33.4 |
| `supervised-renderer-smoke` | passed | 35.6 |
| `semantic-surface-transport-smoke` | passed | 81.0 |
| `semantic-presentation-smoke` | passed | 81.1 |
| `desktop-smoke` | passed | 51.2 |
| `workspace-smoke` | passed | 75.9 |
| `normal-exclusion` | passed | 32.5 |
| `dispatcher-smoke` — KVM repeat | passed | 31.9 |
| `workspace-smoke` — KVM repeat | passed | 53.2 |

Both workspace runs verified **21 keyboard, 410 pointer/Appearance, and 23
graphics frames**, including their retained containment, resource-restoration,
and shell-fallback paths. The workspace image SHA-256 remains
`7ef45ff45e6592ee1399f8922e95a87dc946afcbdb19004464a445e2d1275e9d`,
byte-identical to the preceding graphics checkpoint. The dispatcher increment
therefore adds no change to the desktop image.

Early matrix attempts stopped on a cumulative feature-tier test that still
assumed two development profiles, then an all-feature duplicate-lint-attribute
check. The final source updates the exact three-profile tail and merges the
existing conditional lint allowances without broadening them. The final
complete run above starts after those fixes; earlier partial attempts are not
counted as passing evidence. Native review also corrected the fixture's exact
CMP/RFLAGS predicate and saved-continuation pointer derivation before the
recorded final runs.

The native fault predicate checks the exact address derived from the linked
fixture symbol and the known CMP flags, allowing the processor's fault resume
flag where applicable. It does not accept arbitrary task faults. Published
runtime state and saved continuation cells share one raw-pointer derivation;
no separate Rust mutable reference spans asynchronous native dispatch. The host
transcript validates aggregate native assertions and does not independently
decode the faulting instruction address.

## VirtualBox backend limit

A separate isolated headless probe used the same dispatcher image with
VirtualBox 7.0.16. Fresh UUID `85dd2820-8187-485b-b461-72087e8fd338` had its own
registry, VM directory, private disk, and bounded serial log. Startup failed
before guest execution with **`VERR_SVM_IN_USE`**: host KVM owns the AMD-V
extension. This is an unavailable independent backend result, not a passing
dispatcher test or an observed guest failure.

Cleanup completed for the owned probe VM and disk. Existing VMs, host modules,
and host settings were not changed; the default registry still held only the
existing `Anomalous Dev [Threshold]` VM and no running VMs. The probe's
`result.json`, command log, and VirtualBox diagnostics are retained under
`work/dispatcher-validation/virtualbox-85dd2820-8187-485b-b461-72087e8fd338`
in the development-session directory above. TCG and KVM provide the completed
native evidence for this increment.

## Following gates

Finish the selected kernel prerequisites, then define and prove the versioned
native launch contract. Implement a Reliquary execution envelope against that
contract before a separate Linux ELF admission adapter. Linux process startup
and syscall behavior form another explicit compatibility tier; parsing an ELF
does not establish Linux execution. After the Linux target, develop the
independent Mach-O/Darwin veil. Rich parsers and foreign-platform policy remain
outside the kernel, and no new wire format is frozen in this increment.

The next address-space gate is more than increasing the three-extent limit.
Threshold currently identity-maps the first 4 GiB and links the kernel at
physical/virtual 1 MiB. Kernel page-table and allocator access casts physical
addresses directly to pointers, so a low user remap can redirect privileged
physical-memory access. Prove separation of user virtual geometry from that
access before low ELF mappings, then prove multiple owned branches and rollback.
Temporary supervisor aliases and a durable high-half layout require a later
explicit design; neither is selected by this audit.
