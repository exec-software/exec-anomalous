# ADR 0067: Qualify retained native workspace admission

- Status: Proposed; implemented and qualified as a bounded single-workspace proof
- Date: 2026-10-05
- Related: ADR 0032, ADR 0046, ADR 0047, ADR 0048, ADR 0064, ADR 0066

## Context

The persistent workspace runs the real desktop model, compositor and physical
input adapter in one userspace vessel. Its existing runtime owns a high-address
three-region address space that borrows the bootstrap supervisor branch. The
native application proofs use retained prepared launches, explicit resident
mappings, checked physical access and retained transition-stack ownership.
These are different execution paths, not interchangeable root addresses.

[ADR 0066](0066-model-an-owner-bound-editor-workspace-bridge.md) qualifies the
portable editor/workspace bridge. Native integration remains gated by workspace
admission. The prior high-address workspace RX payload is 88,578 bytes and its emitted
user-stack bound is 61,240 bytes. Neither fits the preceding application's
32 KiB RX and 8 KiB stack profile. Increasing a task count or copying the
portable bridge's two pixel buffers onto that stack is insufficient.

Qualify the real workspace under an explicitly selected retained launch first.
Keep its current GPR/TS execution contract and existing mailbox ABI. Combining
workspace, editor and durable service under one coordinator remains a later
gate, including their different CPU-state and routing requirements.

## Decision

### Select a separate fixed workspace layout

Add `owned_workspace_test`, depending on `workspace_dev`, and a separate
`owned_workspace_runtime` adapter using `owned_execution_group<1>`. The trusted
profile selects exactly one prepared workspace owner. A payload, startup field,
observed mapping count or copied address cannot select or enlarge this profile.

The new workspace artifact uses the same application source with the portable
`native_editor_bridge` feature disabled. Its separately selected link layout is:

| Region | User address | Pages | Permissions |
| --- | --- | --- | --- |
| Instructions and immutable constants | `0x400000` | 32 | User RX |
| Mailbox and private data | `0x800000` | 8 | User RW, NX |
| User stack | `user_stack_top - 65536` | 16 | User RW, NX |

`user_stack_top` retains the existing owned-execution constant, two pages below
the lower canonical user-address limit. The first data page remains the fixed
mailbox. Initialized data starts at `0x801000` and occupies at most the remaining
seven pages. The RX allocation is exactly 128 KiB; the admitted initialized code
extent still comes from the inspected payload length and must fit that bound.
The proof artifact further limits linked initialized/BSS data to three pages
after the mailbox, reserving the remaining pages for wide-data canaries.
Zero padding is not an additional admitted return-instruction extent.

Reserve a guard page immediately before and after each region, and reserve the
low 2 MiB against user mappings. Validate the exact addresses, extents, entry,
stack top and effective W^X permissions before allocation and again before
activation. Keep all earlier one/eight-page RX, two-page RW and two-page stack
profiles unchanged. A generic increase to their limits is not this decision.

Copy the fully inspected initialized RX and RW bytes into their exclusive
leases, preserving the mailbox/data offset. Clear and verify the remaining
bytes, including the whole user stack, before publication. The old workspace
target, linker selection, binary behavior and development image remain separate.
The normal kernel and existing application images must exclude this profile.

### Retain ownership through entry and withdrawal

Use the existing prepare/commit/activate discipline. Preparation retains the
mapping owner and circle attachments without publishing a task. Commit creates
the ready task; registration and unscheduled start retain its exact frame.
Only the completed owner may select its root and transition stack for entry.

The fixed user regions own 56 data frames. The private 16 KiB transition stack
adds four data frames, and the resident page-table tree adds nine table frames:
69 frames in five leases before presentation storage. Retaining the existing
576-frame full-screen stage makes the expected total 645 frames in six leases
(636 data frames and nine table frames). The supervisor physical-access alias
borrows reserved bootstrap tables; its geometry is not an additional RAM grant.
The implementation checks these exact accounting expectations; guest
qualification must independently exercise the resulting admission and release.

Retain every acquired owner and cleanup-progress flag after partial preparation,
registration, admission or release failure. Do not replace explicit cancellation
with fallible `Drop`, discard a partially cleared lease, or remove the physical
alias while a dependent root or continuation remains live. Injected host-test
clear/population failures must exercise real retained rollback and retry paths.

On normal exit, watchdog withdrawal or a contained userspace fault, mark the
exact task terminal and restore bootstrap CR3, TSS.RSP0, CR0.TS and the kernel
continuation before reclaiming its mappings or privilege stack. Withdraw the
terminal execution/frame consumers, unregister roots, clear/release owned storage
and remove the alias only when all dependants are gone. Restore shell input and the
fallback display owner before reporting successful withdrawal. An uncertain
mapping or continuation invariant fails closed while retaining ownership.

### Preserve the existing workspace execution contract

Keep the existing GPR-only soft-float target, disabled red zone and `INT 0xf3`
mailbox calling convention. The new entry points to the relocated mailbox and
starts with the admitted 64 KiB stack. This slice preserves the workspace's
CR0.TS-based FP prohibition/restoration; it does not adopt the application's eager
FXSAVE/FXRSTOR session or qualify extended CPU-state isolation between tasks.

On every gate, exception and user-origin IRQ, validate the complete retained
frame address, current root, selected transition stack, task identity/state and
owner. Normal gate and IRQ returns also validate admitted code/stack return
bounds. Expected guard/NX exceptions instead match their exact scenario frame;
the intentional NX instruction address lies outside RX. Treat invalid userspace flags, mailbox or
return state as attributable workspace faults only after kernel-owned frame
identity has been established. A fabricated or unowned supervisor frame is a
kernel invariant failure, not permission to clean up a guessed owner.

Retain the existing keyboard/pointer delivery and watchdog policy. Yield may
perform its existing bounded idle wait under the established IF/source rules.
Kernel-origin IRQs must not reborrow a runtime already borrowed by its gate.
The timer does not preempt this single workspace into another application.

### Keep input and presentation copies scoped

The workspace retains only its existing input, display, yield and exit rights.
It obtains no storage authority, editor-service route or new launch capability.
Recompute mailbox access from the exact selected retained mapping; a numeric
mailbox address or user-supplied register is never an independent grant.

Use complete checked spans and retain the existing 4 KiB per-copy bound. A
rejected destination or malformed request must not consume queued input or
publish a partial reply. A valid workspace display row remains 3,112 bytes
including its existing header, within that bound. Do not expand copies to expose
the adjacent private initialized data, page tables or another lease.

The 1024 by 768 RGB8 stage remains supervisor-only and exclusively owned. Copy
and validate every row before complete-frame publication; malformed, partial or
out-of-order stages cannot become visible. No userspace framebuffer mapping is
introduced. The retained low root must not assume that bootstrap identity
addresses for the stage or hardware aperture are valid under its CR3.

Where presentation temporarily selects bootstrap CR3, use an explicit IF-clear
scope that validates the actual retained supervisor stack and preserves its
root/TSS/continuation contract. Perform no user copy, task dispatch or ownership
change in that callback. Restore and revalidate the exact selected root before
returning to userspace. Contract or restore failure is fail-stop, not a retry
from an uncertain mapping. The final emitted review must cover this path.

## Qualification and disproof

Qualify actual workspace behavior under the new owner, with the bridge disabled.
The required evidence is larger than an allocation test or a frame counter:

- Inspect the separate static artifact's entry, RX/RW loads, allocated sections,
  initialized data, zero tail and fault markers. Reject W+X, relocations,
  dynamic imports, TLS, unsupported geometry and source extents outside retained
  immutable storage. Post-link inspection must bind exact payload bytes and
  aliases to the kernel and boot image.
- Exercise the real desktop/compositor and physical input. Compare complete
  frames to the existing canonical model, including an initial frame, window
  interaction and an editor input path that exercises the substantial stack.
  Reused rendering remains cross-build evidence, not an independent glyph proof.
- Verify the larger private RW extent, including pages beyond the old 8 KiB
  application boundary, and the full zeroed stack before entry. Negative tests
  cover wrong roots/tasks, guard-crossing spans, malformed copies, permissions,
  overflow and unsupported profile selection without partial mutation.
- Contain the controlled `UD2`, malformed mailbox/flags, guard-write and NX
  execution cases under their exact fault/frame ownership rules. Retain the
  watchdog and normal-exit cases. Each successful withdrawal must restore exact
  resource accounting and working shell fallback; a subsequent launch must
  obtain freshly cleared storage and fresh task/circle ownership. An earlier
  admission's stale bindings must not authorize the new owner.
- Exercise partial construction, population and admission cancellation and
  cleanup-clear retry with unchanged accounting for unrelated owners. A host
  fault injection is not evidence that physical memory failed in the guest.
- Emit strict ordered lifecycle records tied to inspected geometry, observed
  copies/faults and exact restored resources. Reject missing, extra, reordered,
  duplicate or mutated evidence; keep variable timer observations distinct from
  deterministic semantic counts and verify process completion independently.

Bind fresh emitted control/stack inspection to the exact new client and kernel.
Close direct calls, indirect targets and local jump tables; review exceptional
and nonlocal continuations separately. Check complete 64 KiB user/bootstrap and
16 KiB transition-stack bounds, including nested idle IRQ allowance, and admit
no unexpected SIMD/MMX/x87 instructions. A prior workspace graph is a review aid,
not a certificate for relocated or rebuilt executable bytes. Keep Threshold's
384 KiB staging limit and the retained 2 MiB resident boundary unchanged.

Run the new proof at 96 MiB and 320 MiB with the selected QEMU backends. Perform
an isolated VirtualBox boot/frame/lifecycle check when the environment supports
this GPR/TS profile, preserving the existing user VM. A screenshot or harness
event observation alone is not proof of cleanup or complete semantic behavior.
Record platform limitations without weakening an existing CPU or ownership guard.

Serialize builds and VMs under the established memory/OOM limits. Freeze sources,
retain failed attempts and exact artifacts, run all required repository checks
and affected ownership/application/workspace regressions, compare default image
and payload bytes, and restore development images. Describe any artifact changes
precisely rather than inferring equivalence from matching screenshots.

The proof fails if the new geometry is selectable by untrusted metadata, an old
profile silently admits it, a stale frame can select or free an owner, incomplete
copies/pixels publish, faults lose cleanup ownership, or a fresh launch observes
prior private bytes. The final artifact-bound results are recorded in the
[Owned Workspace Admission Log](../../notebook/Development/2026-10-05%20Owned%20Workspace%20Admission%20Log.md).

### Bounded qualification results

The corrected implementation passed the complete workspace/editor trace at
96 MiB and 320 MiB under both TCG and KVM: 32 VM boots, 100 admission/release
cycles and 2,428 interactive frame comparisons. An isolated VirtualBox boot
passed guard/NX retirement and initial rendering with all 786,432 pixels matching;
it does not supply the full interactive VirtualBox lifecycle. All sixteen
required repository checks and affected workspace/ownership regressions passed.

Fresh emitted review binds the corrected kernel and unchanged relocated user
artifact to the exact final image. The first guest exposed an identity-mapped
mailbox-reader assumption that the initial static review missed. The corrected
owned path copies its fixed header through the revalidated supervisor alias and
uses the existing pure decoder; legacy address bounds remain unchanged. Failed
attempts, final artifacts, default-image delta reviews and memory measurements
remain separately recorded. No OOM event occurred.

## Limits and following gates

This profile admits one real workspace vessel under retained ownership. It does
not run three application tasks together, migrate the workspace to eager CPU
state management, enable the portable native editor bridge, grant document or
disk access, or claim that Antimatter's Electron frontend has been ported.

The next composition needs one coordinator for the independently owned workspace,
editor and durable service; explicit task-bound action/observation routes; focus
fencing for delivered actions; and qualified storage for committed/pending editor
frames. Service failure must leave appropriate editor/workspace routes alive.
Those responsibilities are not established by this single-owner admission gate.
