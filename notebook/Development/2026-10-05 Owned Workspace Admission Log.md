---
title: "Owned Workspace Admission Log"
date: 2026-10-05
tags: [native, workspace, ownership, admission, qualification]
status: bounded-native-workspace-qualified
---

# Owned Workspace Admission Log

## Scope

[ADR 0067](../../docs/adr/0067-qualify-retained-native-workspace-admission.md)
adds the opt-in `owned_workspace_test` profile. It runs the existing workspace
under one retained prepared launch with explicit low-address RX/RW regions and
a 64 KiB user stack. It preserves the GPR/CR0.TS execution contract, fixed
`INT 0xf3` mailbox, physical input adapter and complete-row presentation.

This is the native admission prerequisite identified by the
[portable bridge log](2026-10-05%20Editor%20Workspace%20Bridge%20Log.md).
The native editor bridge remains disabled. This gate adds no three-task
coordinator, eager FP context, document service, disk access, filesystem,
Electron frontend or general application launcher.

The selected layout owns 32 RX pages at `0x400000`, eight RW/NX pages at
`0x800000`, and 16 RW/NX stack pages below the existing owned stack top.
The mailbox occupies only the first data page; initialized data starts at
`0x801000`. Six surrounding guard pages and the reserved low 2 MiB remain
unmapped to userspace. The exact geometry is selected by trusted profile code,
not payload metadata. Earlier application and workspace profiles keep their
existing layouts and limits.

The retained launch owns 69 frames in five leases: 56 user data frames, four
transition-stack frames and nine page-table frames. The private 576-frame
presentation stage brings the live total to 645 frames in six leases. The
framebuffer callback switches temporarily to bootstrap CR3 with IF clear on
the retained transition stack, then restores and revalidates the exact root.
It performs no user copy or dispatch. Terminal withdrawal restores bootstrap
CR3, TSS.RSP0, CR0.TS and continuation before clearing and releasing storage.

Every VM boot first admits and retires separate guard-write and NX-execution
owners before the interactive desktop. Their exact fault frames are checked.
Each admission checks zeroed private data beyond the earlier 8 KiB limit and
the bottom of the full user stack, then checks written canaries at retirement.
Ordered lifecycle records expose geometry, population and exact restoration.
Runtime invariant failures retain ownership and fail closed.

## First candidate and alias correction

The first 96 MiB TCG boot completed guard and NX retirement but withdrew the
desktop with `user_fault` before frame 1. Cleanup restored resources and the
shell. A separate GDB run identified the first rite call: the reused header
reader required an identity-mapped address below 4 GiB, while the retained
mailbox uses a validated high supervisor alias. The prior static review missed
that transport contract incompatibility; its no-blocker conclusion was
superseded by the guest result. Both diagnostic runs are retained.

Commit `3ee99fa` adds a fixed 40-byte volatile header copy after exact retained
frame/root/task/mailbox validation, followed by the existing pure decoder.
Legacy transport range checks remain unchanged. Focused kernel tests (542),
feature Clippy and image construction passed. The user ELF/RX/RW bytes are
unchanged. A fresh kernel review and full qualification follow this correction.

## Final-candidate static emitted review

Evidence lives under
an external validation directory (not distributed).
Implementation checkpoint `b77c7b6` was pushed before guest qualification.
The final `source-freeze-v2.json` records 420 non-Markdown files after the
`3ee99fa` correction. Final artifacts and reviews use the `-v2` evidence paths;
the original candidate and failed preflight remain separately retained.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Kernel ELF | 2,680,520 | `858f995ff71dc1aa192833135963574a252e89babf023b8185a3f0cf06126fb3` |
| Kernel flat image | 339,968 | `d4bf85802047df0ef01a89e8aba8edbc64e65f22dfd59cdb823692ef8ceaa9ed` |
| Workspace ELF | 2,299,872 | `2b7948165eb6590e2aaa460a1dc1f67f1ea002d8f6ac01c3adcbfef77fe06771` |
| Workspace RX | 88,690 | `b2bf28918f6ac7e8418b6b2590cee791d53312ac61faa2bffbee8208f4c4900c` |
| Workspace initialized RW | 16 | `814dd7b9784d57c15b9c2972e9b4fd6cf7e164f8162a934bdb2452a413dab1f7` |
| Threshold boot image | 16,777,216 | `07d431becbe325a36403a8491566eed14f782261054b3b8e547952e27bcd1025` |

The fresh user review closes 496 register calls/tails, 25 local jump tables,
1,810 direct transfers and 99 balanced local stack graphs. Its conservative
entry bound is 61,240/65,536 bytes, leaving 4,296 bytes. The intentional jump
at `0x400069` to NX data at `0x807000` is a separately resolved fault boundary.
There are no unresolved executable transfers, dynamic stack adjustments,
red-zone accesses or admitted SIMD/MMX/x87 instructions. Two unreachable
recursive UTF-8 diagnostic edges are excluded with source-invariant binding;
ordinary panic paths remain charged. The previous graph was only a review aid.

The fresh kernel review closes 169 admitted functions, 34 manual indirect
sites, 19 control-flow-proven register definitions and seven local tables.
Bootstrap plus one IRQ uses at most 28,191/65,536 bytes. The rite gate plus
its nested idle IRQ uses 5,310/16,384 bytes; fault and IRQ bounds are 1,567
and 1,111 bytes. No admitted unresolved transfer, cycle, dynamic stack
adjustment or SIMD/MMX/x87 instruction remains. Nonlocal entry/resume and the
temporary presentation root switch have separate instruction-bound reviews.
Fatal Rust panic diagnostics and unreachable compiler-builtins/libm remain
outside the admitted-return graph; structured runtime fatal paths are included.
These bounds assume the reviewed sole-BSP, IF and masked-NMI policy.

All initialized allocated kernel sections match the retained flat image.
The embedded RX/RW spans equal the retained workspace ELF loads and exact fault
markers. Threshold manifest/checksums pass. The kernel leaves 53,248 bytes in
the 384 KiB staging limit and ends resident storage at `0x1a4000`, below 2 MiB.
Static review does not establish guest outcomes.

## Guest qualification

The final image passed all four complete QEMU configurations. Each invocation
record binds explicit accelerator, CPU selection, guest memory and boot-image
SHA-256 to the guarded process scope. No rebuild changed the reviewed image.

| Backend | Guest RAM | Full trace seconds | VM boots | Pixel-checked interactive frames |
| --- | ---: | ---: | ---: | ---: |
| TCG, Intel baseline | 96 MiB | 214.57 | 8 | 607 |
| TCG, Intel baseline | 320 MiB | 222.56 | 8 | 607 |
| KVM, host CPU | 96 MiB | 177.55 | 8 | 607 |
| KVM, host CPU | 320 MiB | 177.56 | 8 | 607 |

Each cell checks 21 keyboard, 465 pointer/Appearance, 26 Graphics and 95 Editor
Lab frames against the existing canonical models. Four terminal probes add
frame records without pixel captures. The separate independent audit validates
809 exact records per cell: 25 admissions/releases, nine desktop sessions and
all guard/NX, malformed mailbox/flags, controlled fault, watchdog, normal exit,
shell-version and pointer-source generation-2 restart tails. Across four cells, this is 32 VM
boots, 3,236 records, 100 admissions/releases and 2,428 interactive frame
comparisons. All 48 retained representative PPMs match corresponding frames
across configurations. This does not mean all compared frames are retained,
or that a reused compositor reference is an independent glyph implementation.

An isolated 256 MiB VirtualBox run passed its strict 16-record initial lifecycle:
guard and NX retirement, desktop admission and first frame. All 786,432 decoded
RGB pixels match the qualified QEMU frame. Private VM configuration/IPC identity
and sampled VM/VBoxSVC cgroup placement match the guarded harness; recorded
cleanup leaves the test VM powered off. This qualifies initial admission and
rendering, not the full interactive VirtualBox trace or graceful retirement of
its final desktop task. Existing user VM registrations were not adopted.

## Repository regression and memory qualification

The corrected source passed full `make check` in 939.32 seconds. The earlier
candidate also passed it in 990.80 seconds, before the guest exposed the alias
contract defect. All fifteen mandatory smoke/exclusion commands also passed, completing all
sixteen repository checks on the corrected source. Both ordinary workspace regressions and the retained-dispatch/application-profile
smokes passed. The 43-job serial qualification batch and six finalization jobs
completed without failure. Normal and workspace development images were restored;
final formatting/diff checks passed and all 420 frozen non-Markdown files still
match `source-freeze-v2.json`.

The 23-artifact default comparison preserves the normal and workspace boot
images, all client RX/RW bytes and all client ELF LOAD geometry/content. The
interactive client's whole ELF changed only outside loaded bytes. Six native/ELF
proof kernel images changed; they are not described as byte-identical. The complete 14-component ELF relaunch
loader/container package also remains byte-identical.

The six changed kernels have a fresh exact delta review against their retained
qualified predecessors. Each has one changed validation/initialization function:
an equivalent permission-loop register/flag rewrite, with a consistent temporary
slot exchange and cold-block relocation in the interactive variant. Call targets
and stack-operation sequences are unchanged. Every other function matches after
control-address mapping and three validation-closure renamings. All remaining
loaded deltas are mapped control pointers, two diagnostic source-line records
across the reviewed set, or decoded NOP padding. Replacing the flat kernel and
updating its mapped entry/checksums reproduces each complete image exactly.
This is delta closure, not a new whole-graph stack search: it preserves only
the preceding reviews' scopes and assumptions and creates no missing numerical
stack certificate. The separately admitted workspace has its own fresh complete
review above. The six older durable/relaunch guest traces were not all rerun;
their clients, payloads and proven kernel deltas are the continuity evidence.

Builds and VMs ran serially with two Cargo jobs and one test thread under a
4 GiB soft limit, 6 GiB hard limit and 512 MiB scope swap limit. A two-second
watcher aborts on host available memory below 4 GiB or any host/scope OOM increase.
Across all attempts, measured peak was 4,295,507,968 bytes (4.001 GiB), and minimum
host available memory was 15,860,256,768 bytes (14.771 GiB). The first full check
recorded 53 soft-limit reclaim events; no hard-limit, OOM or OOM-kill event
occurred. The post-fix full check peaked at 3,547,975,680 bytes (3.304 GiB), and
the 43-job qualification batch peaked at 2,081,456,128 bytes (1.939 GiB), both
with zero soft-limit events. Global host swap observations include other work;
they are not attributed to this task.

Failed early attempts remain retained: two feature-gated helper wiring/lint
failures, a test-only private-selector reference, four stale exhaustive feature
precedence expectations, and the first guest's mailbox-alias rejection. Review
also corrected the expected guard-fault classifier before the first image.
No failed attempt is counted as a successful qualification. Production profile
priority was preserved when correcting its exhaustive test expectations.

## Following gate

One coordinator must retain the independently owned workspace, editor and
durable service. It needs explicit task-bound routes, focus fencing for actions
already delivered, service-failure isolation and qualified storage for committed
and pending editor frames. The two 18,432-byte content frames do not fit in
the workspace's remaining stack margin. The workspace's TS prohibition and
the application's eager CPU-state contract also need deliberate composition.
Neither increasing a task count nor copying the portable bridge onto this
stack establishes that next gate. Full native Antimatter remains product
Milestone 1, beyond this single-workspace admission.
