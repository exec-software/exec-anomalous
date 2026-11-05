---
title: "Anomalous — Decision Register"
date: 2026-10-04
tags: [decisions, adr, architecture, shell, display, desktop, vessels, rites, x86-64]
status: active
domain: architecture
---

# Anomalous — Decision Register

This note is an index and working queue. It does not replace the formal ADRs in
[`docs/adr/`](../../docs/adr/). When a question settles a privilege boundary,
stable format, platform abstraction, or accepted direction, record the result
there and link it here.

## Current direction — 2026-10-04

Accepted [ADR 0045](../../docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md)
selects Antimatter as official editor, Ny as the official language direction for
Anomalous and HAG, and HAG as the compute runtime intended to replace Vulkan.
Native Antimatter running and rendering in userspace is the next major slice
after the kernel application gates. Retained launch ownership now composes with
bounded sustained dispatch under proposed ADR 0047. Proposed ADR 0048 adds the
internal CPU/copy/fault profile; all 28 validation targets passed on 265 unchanged
frozen source files, including two-size TCG/AMD-host KVM proofs. Proposed ADR 0049
qualifies one pinned Ny object/startup composition; all 30 targets passed on 272
unchanged frozen files, including two-size TCG/AMD-host KVM proofs.
Proposed ADR 0050 qualifies a bounded Rust surface/input prototype; all 33 matrix
jobs passed on 291 unchanged frozen files. Proposed ADR 0051 then qualified the
userspace ELF/native loading comparison with 35 matrix jobs and two restoration
jobs on 302 unchanged frozen files. Proposed ADR 0052 qualifies the separate
Linux startup/SYSCALL record-device comparison with 37 matrix jobs and two
restorations on 313 unchanged frozen files. Host Linux Antimatter
remains the behavior reference; Windows/macOS follow later. HAG transport
qualification remains open; existing
verified Rust stays authoritative.
The [direction log](../Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
records the source audit and proposed acceptance. This inserts the native editor
milestone before the foreign compatibility sequence recorded historically below.

Proposed [ADR 0046](../../docs/adr/0046-prove-retained-prepared-launch-ownership.md)
implements the preceding retained-launch slice. Preparation creates no task;
explicit commit publishes one ready task, and cancellation retains ownership
through failed cleanup. The
[Prepared Launch Ownership Log](../Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records its completed tests and the gates identified at that checkpoint.

Proposed [ADR 0047](../../docs/adr/0047-prove-sustained-dispatch-over-retained-launches.md)
implements the preceding composition: two retained launches and high-alias
privilege stacks survive 64 preemptions, one exact UD2 fault, and 16 survivor
ticks. Narrow unstarted scheduler withdrawal preserves queue order and sequence
cursors while rejecting any global current task or reservation. Stack
clear/release failures retain retry ownership; failed mapping revalidation
remains retained fail-closed without a recovery claim. Both owners survive
until both tasks are terminal. Focused tests and two-size TCG/KVM evidence
passed; all 26 regression targets qualified across documented phases, with
258 final frozen files unchanged after a narrow display-capture harness fix.
The evidence and original diagnostic limit are retained in the
[Owned Sustained Dispatch Log](../Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md).
This remains a proposed fixture boundary, not a stable ABI or general loader.

Proposed [ADR 0048](../../docs/adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
adds an internal eager x87/SSE2 owner, current-task copies bounded to 4096 bytes,
and selected #DE/#UD/#GP/#PF handling through last-current fault cleanup.
Task return admission stays separate from attribution of a terminal user fault.
The [Application Profile Log](../Development/2026-10-03%20Application%20Profile%20Log.md)
records all 28 passed targets, exact CPU/resource restoration, unchanged frozen
source, and the preceding-checkpoint normal/workspace image match.
This does not accept TLS, AVX/XSAVE, a complete untrusted-application fault
profile, or native Ny/Antimatter integration.

Proposed [ADR 0049](../../docs/adr/0049-qualify-a-bounded-freestanding-ny-object.md)
keeps the pinned compiler's object intact and supplies only an inspected scalar
startup/runtime shim. It targets initialized-data and zero-fill evidence,
integer/f64 work interrupted inside generated code, two attributable failure
paths and successful peers, then full retained-owner cleanup. All 30 targets
passed on 272 unchanged frozen files. The
[Ny Object Log](../Development/2026-10-03%20Ny%20Object%20Log.md) records final
TCG/KVM evidence, unchanged restored normal/workspace images and zero OOM events.
This is not a general Ny target, hosted runtime port, native artifact format,
or Antimatter implementation.

Proposed [ADR 0050](../../docs/adr/0050-prove-a-bounded-native-application-contract.md)
keeps editor policy in independent Rust userspace and binds opaque copied
surface/input channels to exact task generations. The model ports Antimatter's
selection replacement, theme preservation and undo into a bounded ASCII
software surface. Normal/fault scenarios and one deferred bootstrap presentation
passed with all 33 matrix jobs on 291 unchanged frozen files, including two-size
TCG/KVM and capture proofs, recorded in the
[Native Application Log](../Development/2026-10-03%20Native%20Application%20Log.md).
The host ELF build container establishes no ELF execution ABI. Full editor,
Ny frontend, physical input, files and stable service contracts remain open.

Proposed [ADR 0052](../../docs/adr/0052-prove-linux-startup-and-record-device-syscalls.md)
keeps the existing native service while one inspected producer uses a real Linux
initial stack and SYSCALL. Its fixed nonblocking record devices, atomic input
consume, poll-copy guarantees and restored entry controls passed two-size
TCG/KVM qualification and matching captures, recorded in the
[Linux Application Log](../Development/2026-10-04%20Linux%20Application%20Log.md).
The syscall adapter is feature-private proof-controller policy, not a production
kernel compatibility layer. At that checkpoint the userspace veil was the next
architectural gate under ADR 0008; Linux pipes, stock libc, signals, TLS and
Electron remain outside this proof.

The preceding proposed [ADR 0053](../../docs/adr/0053-prove-a-supervised-userspace-syscall-veil.md)
implements that policy in a third independent userspace vessel. Sealed
blocked-caller transactions preserve native ownership while inert staging and
reply preflight make publication atomic. Dependency-fault containment discards
unpublished effects and continues the native service. All 39 matrix jobs and two restorations passed; evidence is in the [Userspace Veil Log](../Development/2026-10-04%20Userspace%20Veil%20Log.md).
Fixed startup remains a controller fixture; ADR 0054 below extends membership.
Application runtime capabilities remain open.

The preceding proposed [ADR 0054](../../docs/adr/0054-prove-a-shared-multi-caller-userspace-veil.md) qualifies two independent Linux editor
producers and two native services sharing one supervised veil. Exact sealed
membership, per-caller routes and copied-memory isolation preserve the native
baseline. A caller fault leaves its peer progressing; a veil fault revokes both
dependents and discards its staged effect while both native services survive.
All 41 qualification checks and two image restorations passed, including TCG/Intel and
KVM/AMD at both guest sizes with capture comparison. All twelve preceding
embedded artifact blobs remain unchanged and no OOM occurred. The
[Shared Veil Log](../Development/2026-10-04%20Shared%20Veil%20Log.md) records the frozen-source and compiled-code evidence.
This is a fixed two-member serial service, not full Antimatter/Electron or a
general process/runtime service.

The preceding proposed [ADR 0055](../../docs/adr/0055-prove-native-startup-and-private-userspace-arenas.md) implements
versioned copied startup and separately owned private arenas for a bounded
native editor client. Kernel task/channel bindings remain authoritative; only
the explicit arena launch/circle profile permits four mappings. All 43 qualification checks and two image restorations passed, recorded in the [Native Runtime Log](../Development/2026-10-04%20Native%20Runtime%20Log.md).
Proposed [ADR 0056](../../docs/adr/0056-prove-task-bound-native-document-open-and-save.md) implements the current
RAM document service increment. Exact saved-snapshot replies preserve newer
edits; unknown outcomes stay dirty and retain the pending snapshot. Bounded
native qualification passed; the
[Native Document Log](../Development/2026-10-04%20Native%20Document%20Log.md)
records the evidence. Close/reopen retains bytes
only while the same service lives. Proposed [ADR 0057](../../docs/adr/0057-prove-elf-loading-of-the-native-document-contract.md)
qualifies the bounded ELF comparison using the unchanged native client and
verified zero-tail compaction within existing loader bounds. The
[ELF Document Log](../Development/2026-10-04%20ELF%20Document%20Log.md)
records the completed qualification. Proposed
[ADR 0058](../../docs/adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md)
adds native editor replacement while the same RAM service retains committed
bytes. The service changes document authority in userspace after checked
successor metadata; copied document records cannot select their own authority.
All 50 qualification checks and two development-image restorations passed; the
[Native Relaunch Log](../Development/2026-10-04%20Native%20Relaunch%20Log.md)
records completed bounded qualification.
Proposed [ADR 0059](../../docs/adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md)
implements retained-service replacement through the userspace ELF loader against
that native baseline. The 222-record oracle checks each route independently;
all 52 qualification checks and two development-image restorations passed. The
[ELF Relaunch Log](../Development/2026-10-04%20ELF%20Relaunch%20Log.md)
records completed bounded qualification.
Scoped filesystem persistence with explicit durability semantics follows. No
save across service destruction is claimed.

## Accepted constraints

| ADR | Decision | Consequence for current work |
| --- | --- | --- |
| [0001](../../docs/adr/0001-people-and-machines-are-coequal-clients.md) | People and machines are coequal clients | The shell and AI clients must project the same operations, policy, and results. |
| [0002](../../docs/adr/0002-threshold-owned-modular-boot.md) | Threshold is the owned modular boot path | Kernel work continues without adding GRUB as an architectural dependency. |
| [0003](../../docs/adr/0003-rust-assembly-and-code-style.md) | Contained assembly, K&R, and `snake_case`; language designation superseded by 0045 | New code keeps unsafe and assembly invariants narrow and documented. |
| [0045](../../docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md) | Antimatter, Ny, and HAG native userspace direction | Qualify the native editor stack after kernel gates; designation alone does not establish readiness or authorize privileged migration. |
| [0007](../../docs/adr/0007-occult-vocabulary-and-accessible-neumorphism.md) | Precise occult vocabulary and accessible neumorphism | A new shell term is not coined casually; interactive state cannot rely on pixels or depth alone. |
| [0011](../../docs/adr/0011-prove-the-first-circle-with-a-fixed-x86-64-prototype.md) | Prove one fixed test-only circle before generalizing isolation | General circle policy must build on exact ring-3, paging-denial, teardown, and clean-replacement evidence without treating the diagnostic gate as ABI. |
| [0012](../../docs/adr/0012-prove-bounded-bootstrap-memory-ownership.md) | Prove bounded bootstrap frame and address-space ownership | General memory work must preserve complete input, reserved-wins normalization, allocator-bound leases, mapping ownership transfer, non-W+X policy, and verified reclamation without treating this BSP-only manager as a heap or isolation boundary. |
| [0013](../../docs/adr/0013-prove-bounded-interrupt-time-and-input-delivery.md) | Prove bounded returning IRQ, time, and private physical-input delivery | Future platform backends must preserve typed source generations, same-timebase ordering, explicit loss and controlled resynchronization, bounded IRQ work, output-only diagnostics, and the separation between hardware input and authenticated human/AI semantic authority. |
| [0014](../../docs/adr/0014-prove-fixed-attributable-sigil-gated-ipc.md) | Prove fixed attributable sigil-gated IPC | General task, circle, sigil, and IPC work must preserve kernel-derived provenance, task-local authority, copied-message immutability, fail-closed generational records, and destructive cleanup without treating the feature gate or synthetic roles as ABI or authentication. |
| [0015](../../docs/adr/0015-prove-bounded-preemption-and-user-fault-containment.md) | Prove bounded preemption and user-fault containment | Production scheduling must preserve inseparable task/root/context/TSS-stack ownership, exact frame validation, static unexpected-fault diagnostics, peer progress after contained task failure, and successful destructive cleanup without treating the two-tick harness as policy or ABI. |
| [0018](../../docs/adr/0018-prove-bounded-vessel-supervision-and-restart.md) | Prove bounded vessel supervision and restart | Production supervision must preserve kernel-owned observations, exact generational identity, safe kernel-stack replacement checkpoints, destructive withdrawal, restart budgets, and complete cleanup without treating the fixed relationship as a service runtime or stable ABI. |
| [0019](../../docs/adr/0019-define-first-supervised-vessel-rite-abi.md) | Define and prove the first supervised-vessel rite shape | Future native clients use the same fixed-width, task-bound rites on x86-64; IA-32 execution and the 32-bit Win32 ABI terminate in a user-space veil, while the accepted gate remains a removable one-task proof rather than a normal-image syscall ABI. |
| [0020](../../docs/adr/0020-compose-a-fixed-supervised-shell-vessel.md) | Compose a fixed supervised shell vessel | Production shell-vessel work must preserve exact task-bound rite channels across scheduling, destructive restart, terminal supervisor wake, coequal client semantics, privacy-safe evidence, and complete reclamation without treating the fixed projections as Canticle or a stable ABI. |
| [0021](../../docs/adr/0021-prove-bounded-private-shell-session-ingress.md) | Prove bounded private shell-session ingress and waiting | A later session host must preserve bounded semantic actions, private move-only storage, FIFO backpressure, exact generational replacement, and lost-wake-safe waits without treating origin labels as authentication or this host model as a kernel ABI. |
| [0022](../../docs/adr/0022-prove-bounded-native-session-input-and-exact-wake.md) | Prove bounded native session input and exact wake | Native session work must preserve physical/semantic separation, exact pending and wake correlation, copy-before-consume, reply-header-last publication, route-close clearing, privacy-safe evidence, and feature exclusion without treating origin as authentication or this provisional rite as stable ABI. |
| [0023](../../docs/adr/0023-prove-bounded-authenticated-source-session-routing.md) | Prove bounded authenticated-source session routing | A later session host must preserve controller-only admission/revocation, private router incarnation, exact source/focus/route binding, fixed human A-to-B and machine-B topology, returned private ownership, and coordinated shared-B retirement without treating the host assertion as credential authentication or local attribution as native transport. |
| [0024](../../docs/adr/0024-prove-bounded-user-space-display-presentation.md) | Prove bounded user-space display presentation | A later compositor must preserve kernel-exclusive framebuffer ownership, copy-once exact patch validation, failure-atomic rejection, task-bound display authority, privacy-safe evidence, and feature exclusion without treating pixels as semantic desktop state or the provisional RGB8 body as a stable graphics ABI. |
| [0025](../../docs/adr/0025-prove-bounded-canonical-semantic-desktop-scene.md) | Prove a bounded canonical semantic desktop scene | A later renderer and desktop service must preserve atomic privately branded revisions, exact accessible/machine projection parity, explicit label disclosure, non-authoritative action/focus references, fixed resource budgets, and failure-atomic ownership without treating the host fixture as a general scene graph or runtime. |
| [0026](../../docs/adr/0026-compose-a-fixed-supervised-semantic-renderer.md) | Compose a fixed supervised semantic renderer | Later compositor and desktop work must preserve redacted deterministic scene derivation, bounded RX renderer images, kernel-exclusive staging and presentation, exact fault withdrawal, fresh supervised replacement, terminal completion, whole-frame publication, privacy-safe evidence, successful-path complete reclamation, and failed-clear quarantine without reuse or false success, without treating this fixed proof as a window server or live scene service. |
| [0027](../../docs/adr/0027-prove-bounded-transactional-semantic-surface-publication.md) | Prove bounded transactional semantic-surface publication | Later native desktop work must preserve one committed revision plus one private successor, complete-owner backpressure, bounded redacted snapshots, privately branded move-only freshness and authority, one pre-attestation renderer replacement, current action/focus validity until publication, and atomic human/accessibility/machine publication only after an exact trusted-host receipt. The accepted proof is host-only: that receipt does not verify a renderer, pixels, or scanout, and its local Rust snapshot is not a live transport or stable ABI. |
| [0028](../../docs/adr/0028-prove-bounded-native-semantic-surface-transport.md) | Prove bounded native semantic-surface transport | Later composition must preserve the explicit 528-byte little-endian redacted body, separate submit/read rights, exact task/endpoint and freshness binding, copy-before-parse and copy-before-consume ownership, reply-header-last publication, privacy-safe clearing, and complete teardown. Delivery alone is not renderer readiness, presentation, scanout, acknowledgement, or semantic publication. |
| [0029](../../docs/adr/0029-prove-bounded-native-semantic-render-and-present-composition.md) | Prove bounded native semantic render-and-present composition | Later deployment must preserve the live retained transaction, exact ordinal-61 copied body, narrow ring-3 renderer authority, complete supervisor-only staging, teardown-before-presentation, full framebuffer readback, independent pixel evidence, and private exact publication controls. This successful-attempt conformance bridge is not scanout/page-flip/GPU evidence, a cross-process presentation protocol, a general desktop, a deployed service, or a stable ABI; its framebuffer-to-host-publication crash gap remains open. |

## Proposed directions, not yet accepted

[ADR 0030](../../docs/adr/0030-prove-bounded-presentation-recovery-coordination.md)
defines a host-only presentation-recovery prerequisite. Its coordinator retains
the actual surface transaction while public semantic reads are unavailable,
and permits one fresh presenter to redraw the same successor. Native service
deployment and loss of the canonical owner remain separate work.

ADRs [0004](../../docs/adr/0004-capability-oriented-hybrid-kernel.md),
[0005](../../docs/adr/0005-shared-semantic-control-plane.md),
[0006](../../docs/adr/0006-reliquary-and-covenant.md),
[0008](../../docs/adr/0008-isolate-foreign-platform-compatibility.md),
[0009](../../docs/adr/0009-separate-ai-semantics-from-hardware-mechanisms.md),
[0010](../../docs/adr/0010-canticle-secure-semantic-command-language.md),
[0016](../../docs/adr/0016-adopt-ny-as-anomalous-implementation-language.md), and
[0017](../../docs/adr/0017-eidolon-lossless-framebuffer-traces.md)
remain proposed. Experiments may explore them, but notes must not report them
as implemented or irreversible commitments.

## Open decisions for the shell path

| Question | Why it matters | Evidence needed | Status |
| --- | --- | --- | --- |
| Where does the first interactive evaluator execute? | Running it in ring 0 is fast for bring-up but has no meaningful containment. | Keep the implemented five-command harness bounded; prove circles and vessel launch before a real evaluator. | proposed in ADR 0010 |
| What is the canonical representation of a command? | Human syntax and machine requests must not acquire divergent semantics. | Prototype text-to-AST-to-invocation round trips and direct structured invocations. | proposed canonical AST in ADR 0010 |
| What is the minimum first circle proof? | A cosmetic ring-3 transition would not establish memory isolation or supervised failure. | Test exact supervisor-read and NX denials, clearing, and a clean third generation without defining general process machinery. | accepted and demonstrated in ADR 0011 |
| What is the minimum trustworthy allocation input? | Raw firmware ranges can overlap, wrap, truncate, and cover live kernel or device memory. | Require a complete handoff, normalize conservatively, reserve occupied extents, and grant only allocator- and generation-tracked whole frames. | accepted and demonstrated in ADR 0012 |
| What is the minimum trustworthy external-input proof? | Returning interrupts, device bytes, time, layout, and semantic authority become dangerous if collapsed into one path. | Prove bounded PIC/PIT/8042 delivery, exact loss, IRQ1 containment, bounded device reinitialization with fresh source generation, private physical records, separate layout, final masking, and no AI or serial-input bypass. | accepted and demonstrated in ADR 0013 |
| What is the minimum trustworthy task-to-kernel IPC proof? | A host-only model cannot prove that ring-3 registers or memory fail to forge provenance or authority. | Bind fixed task/circle/sigil/IPC tables to allocator-owned one-shot ring-3 tasks, task-local bindings, copied FIFO messages, destructive replacement, and exact cleanup without claiming scheduling or stable ABI. | accepted and demonstrated in ADR 0014 |
| What is the minimum trustworthy scheduling proof? | Cooperative one-shot tasks cannot show that real preemption, multiple saved contexts, and task-local failure compose safely. | Use exactly two one-task circles, real PIT preemption, distinct allocator-owned privilege stacks, one exact contained user `#UD`, peer continuation, strict teardown, and no general scheduling claim. | accepted and demonstrated in ADR 0015 |
| What is the minimum trustworthy supervision proof? | A faulted task cannot simply be reset without preserving stale state or erasing its active stack. | Preserve one supervisor, consume one exact kernel observation, replace one vessel generation at a safe kernel-stack checkpoint, and prove clean completion and teardown. | accepted and demonstrated in ADR 0018 |
| What is the minimum native rite-gate proof? | A host-only prefix cannot show that ring-3 state fails to forge task identity or rights, or that terminal exit avoids returning to dead state. | Use one fixed x86-64 shell-task fixture and mailbox for exact rejection, commit, would-block, terminal-return, privacy, and teardown evidence without claiming a stable syscall ABI. | accepted and demonstrated in ADR 0019 |
| What is the minimum shell-vessel composition proof? | Separate scheduler, supervision, and rite images cannot show that preemption, destructive restart, channel lifetime, coequal projections, and terminal supervisor wake compose. | Run two exact shell-vessel incarnations around one real PIT and contained fault; require fresh rite sequence, equal human/machine console semantics, no return to the exited replacement, strict privacy, and complete reclamation. | accepted and demonstrated in ADR 0020 |
| What is the minimum private session-input and wait contract? | A global key stream, ambient history, or check-then-sleep wait would leak input or lose/wrongly replay delivery across sessions. | Host-test bounded semantic actions, private move-only storage, explicit submit, FIFO backpressure, exact session/consumer/route-bound waits, close/restart invalidation, and content-free observations. | accepted and demonstrated in ADR 0021; the native bridge is accepted in ADR 0022 and authenticated-source routing is accepted in ADR 0023 |
| What is the minimum native session-input and exact-wake proof? | Reusing physical input, consuming before user copy, retaining a rite guard across scheduling, or waking by task alone would collapse authority domains or lose/misroute private input. | Add a distinct versioned semantic-session rite, owned pending correlation, copy-before-consume reply, exact task-local readiness, FIFO-tail wake, route-closure interruption, strict privacy, and normal-image exclusion. | accepted and demonstrated in ADR 0022; authenticated-source routing remains separate and is accepted in ADR 0023 |
| What is the minimum authenticated-source routing proof? | An origin label, principal number, copied source identity, or stale prepared action must not select authority or cross revocation and focus changes. | Host-test two exact routes and two source slots, one human A-to-B focus transfer, one machine-explicit B binding, move-only authority, exact per-source sequencing, and close-before-transfer queue clearing without claiming credential authentication. | accepted and demonstrated in ADR 0023 |
| What is the minimum user-space display-presentation proof? | Mapping the raw framebuffer, trusting caller geometry, or treating pixels as machine state would collapse device ownership, memory safety, privacy, and semantic authority. | Give one ring-3 task only display-update and exit rights; copy and validate exact bounded RGB8 patches; prove full-aperture failure atomicity, exact pixels, terminal teardown, and normal-image exclusion. | accepted and demonstrated in ADR 0024; bounded semantic-renderer composition is accepted in ADR 0026, the host transaction boundary in ADR 0027, copied native surface delivery in ADR 0028, and their one-successful-attempt composition in ADR 0029 |
| What is the first canonical desktop scene contract? | Separate human pixels and machine APIs would drift in state, focus, actions, privacy, and accessibility. | Commit one bounded revision and deterministically derive both an accessible render plan and a machine-readable projection with identical action identities and explicit disclosure policy. | accepted and demonstrated in ADR 0025; bounded supervised rendering is accepted in ADR 0026, host-side transactional publication in ADR 0027, one fixed copied/versioned native projection in ADR 0028, and their bounded composition in ADR 0029, while a production native scene/window service remains open |
| What is the minimum supervised semantic-renderer composition? | Per-row display atomicity would expose a mixed frame after a renderer fault, while moving scene authority into ring 0 would enlarge the trusted boundary. | Generate a stripped capsule from the actual paired scene projection, render it in a fixed separately linked ring-3 image, stage one complete frame per generation, discard the faulted half-frame, and publish only after the successful replacement exits. | accepted and demonstrated in ADR 0026; ADR 0029 separately composes one successful live ADR 0027/0028 transaction through supervised rendering and verified presentation, while deployed service lifecycle remains open |
| What is the minimum transactional semantic-surface publication contract? | Publishing semantics on dequeue or renderer readiness could expose machine/accessibility revision N+1 while a person still sees revision N; overwrite or stale retry could also lose the sole private candidate. | Hold one complete private successor under strict owner-returning backpressure; bind each step to exact move-only control, endpoint, lease, prepared-presentation, and receipt ownership; permit one fresh retry; and publish all semantic projections only after an exact trusted-host receipt. | accepted and demonstrated host-side in ADR 0027; ADR 0029 binds one retained transaction to exact native renderer and framebuffer evidence before publishing, but cross-process acknowledgement and the pixels-versus-semantics crash gap remain open |
| What is the minimum native semantic-surface transport? | Serializing ADR 0027's private Rust snapshot or sharing mutable memory would freeze implementation layout, reopen mutation races, and let public correlation values blur into authority; treating delivery as presentation would also publish a false guarantee. | Define one explicit fixed version-1 redacted body, copy it from one narrowly granted source into one kernel-owned slot, bind it to exact task/endpoint and surface freshness, then copy it once to a separately granted consumer before consumption. | accepted and demonstrated in ADR 0028; delivery is not rendering, presentation, scanout, acknowledgement, or semantic publication |
| What is the minimum native render, presentation, and semantic-publication composition? | Independently accepted transport, rendering, framebuffer, and host-transaction proofs do not show that one exact private successor crosses every boundary without premature pixel or semantic publication. | Retain the live transaction, transport its exact ordinal-61 body once, render 768 rows into a private stage, tear down fallible native state, verify the complete framebuffer twice, then consume only the retained private host controls to publish exactly 2/2. | accepted and demonstrated for one successful attempt in ADR 0029; early-exit refusal is structural/component evidence rather than a negative QEMU run, and deployed cross-process service protocol remains open |
| Are pipelines byte streams, typed values, or both? | This determines composition, compatibility, errors, and injection risk. | Exercise realistic administration, text, data, and foreign-command tasks. | open |
| How are effects and required sigils declared? | Ambient authority would violate the intended security model. | Define previewable effect sets and test denial, attenuation, cancellation, and audit behavior. | open |
| Which language constructs are pure? | Evaluation must be understandable before state-changing work begins. | Separate parsing, expansion, planning, authorization, and execution in a prototype. | open |
| How are secrets and governed context represented? | History, diagnostics, interpolation, and AI context can leak private data. | Threat-model redaction, lifetime, disclosure labels, and non-persistence defaults. | open |
| What is the shell called? | A platform term creates a durable abstraction and vocabulary burden. | Evaluate **Canticle** alongside its proposed semantic boundary and conformance evidence. | proposed in ADR 0010 |

## Interactive desktop development proposal

Proposed [ADR 0031](../../docs/adr/0031-add-an-interactive-bootstrap-desktop-development-image.md)
adds a removable native-rendered launcher and returning bootstrap shell for
hands-on development. It preserves ordinary boot and terminal proof precedence.
Persistent native service deployment, authenticated presentation, and semantic
action/focus policy remain separate decisions. See the
[development log](../Development/2026-09-26%20Interactive%20Desktop%20Development%20Log.md)
for implementation and validation evidence.

## Native workspace development proposal

Proposed [ADR 0032](../../docs/adr/0032-start-a-native-window-workspace-and-compositor-service.md)
starts a separate persistent native window owner and software compositor above
the kernel. Four public demo windows and two workspaces exercise reversible
focus, window lifecycle, floating/tiling layout, and declarative customization.
Privately branded local endpoints grant the same typed operations to human and
machine clients; preparing a candidate requires observation as well as its
operation grant. An origin tag or window ID is not authority, and these local
endpoints do not provide external agent IPC.

The first native failure policy withdraws the service and returns to the
bootstrap shell after resource reclamation. It discards workspace state rather
than claiming ADR 0030's surviving-coordinator recovery. Native smoke passed
21 exact frames and all five termination scenarios with full resource
restoration and working shell fallback. The full 20-target validation matrix
passed; the
[workspace development log](../Development/2026-09-26%20Native%20Workspace%20Development%20Log.md)
separates measured results from intended contracts. Application surface
admission, independent client supervision, private content, semantic IPC,
text input, durable profiles, and GPU composition remain open decisions.

## Decision capture rule

For every candidate, record context, alternatives, predicted consequences, a
reversal plan, and the evidence that would disprove it. Use the
[Decision Note template](../Templates/Decision%20Note.md) while exploring; then
promote platform-level decisions to a numbered ADR.

## Native theme and pointer shell proposal

Proposed [ADR 0033](../../docs/adr/0033-build-the-first-native-themes-and-pointer-window-shell.md)
uses the first official Light/Dark themes to define a validated, component-based
appearance contract. The native service now owns mouse gestures, a task panel,
shared paint/hit geometry, and bounded atomic command batches. A copied pointer
ABI and opt-in PS/2 AUX backend supply mechanism below the user-space policy.
The `desktop` shell command starts a fresh owner after withdrawal; state
preservation remains separate. Login/audio roles are unavailable until their
runtime services exist. See the [theme and pointer log](../Development/2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md)
for measured evidence; this proposal does not amend accepted fixture ADRs.

## Shared native frame assets proposal

Proposed [ADR 0034](../../docs/adr/0034-build-shared-native-frame-assets-and-canonical-control-states.md)
keeps original compiled control vectors, host reference exports, and native
composition on one source of geometry. Independent decoration and frame
components validate against the complete appearance before a revision can
publish. Pointer pressed state is canonical and follows the same local
human/machine grants and atomic presentation boundary as window actions.

The official Dark theme follows Antimatter's local Clay reference; Light
interprets its roles with readable warm light surfaces. SVG sheets and their
manifest remain internal design references, with no external runtime loader or
stable asset format. Native smoke, component checks, and the complete 20-target
matrix passed. See the [Clay Frame Assets Log](../Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md)
for current evidence. Accepted fixture ADRs remain unchanged.

## Built-in Appearance app proposal

Proposed [ADR 0035](../../docs/adr/0035-add-a-built-in-appearance-app-with-shared-choice-state.md)
puts explicit settings choices and keyboard selection inside the existing
canonical workspace owner. The app is a fixed public-content singleton;
opening, selection, and activation require the grants corresponding to their
actual effects. Human, machine, and accessibility views share the same option
identities, preferences, and window state. Component changes retain unrelated
overrides; validators can reject incompatible candidates without partial
mutation. Session-local settings introduce neither storage nor a stable app ABI.

The native owner continues after a narrow allowlist of bounded geometry,
capacity, theme, and layout rejections. Unexpected source, authority,
transaction, and presentation errors still withdraw through containment.
This changes the earliest development failure policy without amending accepted
fixture ADRs. All 20 validation targets passed with final component checks,
21 native keyboard frames, and 337 pointer/Appearance frames across two
sessions. The [Appearance App Log](../Development/2026-09-26%20Appearance%20App%20Log.md)
records the evidence. Private console content remains a separate boundary:
move-only private text owners and disclosure-aware views must precede adding
sensitive input, rather than extending the public `Copy` snapshot with text.

## Desktop click lifecycle proposal

Proposed [ADR 0036](../../docs/adr/0036-activate-desktop-controls-on-pointer-release.md)
makes native buttons activate on matching release. The workspace owns control
identity and armed feedback, while the user-space adapter requires source,
revision, target, and topmost-hit continuity. Armed state confers no authority;
ordinary commands retain their complete grants and presentation rules. Every
successor must explicitly rearm a continuing press, preventing another actor's
scene change from preserving stale pressed feedback. Keyboard input, loss,
chords, or release elsewhere cancel. New sources first observe all buttons up.

No kernel mechanism or stable ABI is added. All 20 validation targets passed,
including 21 native keyboard frames and 410 pointer/Appearance frames with
73 new release-gesture frames. The [Desktop Click Lifecycle Log](../Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md)
records validation and the development-desktop checkpoint before returning to
foundational task/wait and supervised-service work. Earlier records remain
historical evidence.

At that desktop checkpoint, the next foundation candidate was a separate
bounded dispatcher proof with sustained preemption and task-local fault
containment. Proposed
[ADR 0039](../../docs/adr/0039-prove-bounded-sustained-native-dispatch.md) now
implements that experiment; its current evidence is recorded below. The
desktop ADR accepts neither a general scheduler nor a production vessel
lifecycle.

## Ready pointer motion proposal

Proposed [ADR 0037](../../docs/adr/0037-batch-ready-desktop-pointer-motion.md)
allows the user-space pointer adapter to incorporate at most 16 already-ready
continuous same-button packets into one semantic update. Every incorporated
raw sequence and generation is validated. Motion clamps to the viewport after
each packet, while drag and resize retain the original capture anchor. Press,
release, chord, loss, keyboard, and source boundaries stay separate; at most
one untouched fetched record is retained for the next loop turn. No deliberate
wait collects a batch.

One bounded command batch still requires the ordinary grants, exact revision,
and a complete-frame acknowledgement before shared canonical publication.
No physical-input ABI, kernel queue, or privilege boundary changes. The change
explicitly permits fewer displayed intermediate positions under queued motion;
it does not discard button/key semantics or bypass raw validation. The
[Mouse Responsiveness Log](../Development/2026-09-26%20Mouse%20Responsiveness%20Log.md)
records final validation: all 20 targets and an additional complete KVM
workspace smoke passed, each native run verifying 21 keyboard and 410
pointer/Appearance frames. Five sustained-input captures matched exact expected
frames. Sustained-motion timing is
load-sensitive, and no real-time claim is made. The user subsequently selected
the native window-content slice below; filesystem and file-app work remain
later choices.

## Native window content surfaces proposal

Proposed [ADR 0038](../../docs/adr/0038-add-native-window-content-surfaces.md)
adds bounded validated opaque RGB8 layouts and immutable borrowed frames above
the kernel. Caller-owned canvases produce raster content; canonical windows
publish its public descriptor/label and content sequence. A distinct `present`
grant and move-only owner bind replacement to an exact client generation and
window incarnation. Preparation and one-successor backpressure preserve the
existing exact-revision, complete-frame acknowledgement boundary.

The compositor validates complete immutable bindings before presentation, then
uses the same retained frames for all rows. Sampling is restricted to the
shared client viewport, retaining chrome and occlusion. The Graphics Lab
singleton runs within the existing trusted workspace vessel; it provides no
independent task, external producer identity, private-content policy, general
semantic content tree, shared-memory transport, or stable graphics ABI.

Linux-first TRI-ABI is a future compatibility direction under ADR 0008. macOS
has a separate Darwin/XNU foundation and does not inherit Linux's ABI. Native
rendering does not implement ELF/syscall support, Wayland, X11, or another
foreign platform. All 20 validation targets and an additional complete KVM
workspace smoke passed, including 23 exact graphics frames in each workspace
run. Native timing and an exact sustained-input capture also passed. The
[Window Content Surfaces Log](../Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md)
records final evidence without promoting this proposal to a stable graphics ABI.

## Sustained native dispatch and execution readiness proposal

At the September 26 checkpoint, the user selected kernel foundations before
the native execution boundary, Linux ELF first, and macOS afterward. Proposed
[ADR 0039](../../docs/adr/0039-prove-bounded-sustained-native-dispatch.md) adds a
separate removable two-task GPR-only proof with 64 real PIT switches, one exact
known fault, and 16 further timer ticks with measurable sole-survivor progress.
The no-peer path must retain the running context without advancing the
dispatch sequence. Safe continuation, verified clearing, complete reclamation,
and normal-image exclusion remain required. Final TCG and KVM runs passed the
exact 14-record transcript with 80 PIT ticks, 66 dispatches, and restoration of
65,112 free frames. All 23 validation targets passed on unchanged frozen source,
including normal-image exclusion and retained desktop regressions. An isolated
VirtualBox attempt was blocked by host KVM ownership of AMD-V and cleaned up.
The existing accepted scheduler proof and desktop behavior remain separate;
ADR 0039 remains Proposed.

The [execution-readiness audit](../../docs/architecture/execution-readiness.md)
corrects the assumption that the native ABI/artifact format is finished.
Implemented rite prefixes are experimental; canonical semantic invocation
encoding, a general versioned launch contract, and Reliquary execution framing
remain open. Current address-space geometry, three-extent capacity, GPR-only
contexts, fixed user copies, and exact-fixture fault handling are not enough
for arbitrary Linux ELF programs.

The order at that checkpoint was the selected kernel prerequisites, native launch
contract, Reliquary execution envelope, a separate Linux ELF adapter and syscall
personality, then an independent Mach-O/Darwin veil. Rich artifact parsing and
foreign policy stay outside the kernel; requested rights never become grants
through a parser. This records direction without inventing wire fields or
accepting ADRs 0006/0008. The
[Sustained Native Dispatch Log](../Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md)
holds the final measured evidence.

## Passwordless entities and scoped administration proposal

Proposed [ADR 0040](../../docs/adr/0040-separate-entity-identity-authentication-and-authority.md)
records the user's direction: native entities for humans and agents without
username/password authentication, with future biometric user verification.
A stable entity is distinct from replaceable authenticators, a live task/session,
and separately granted sigils. Administrative maintenance and recovery powers
remain necessary and scoped; identity carries no universal administrator bit.
Biometrics activate a protected key locally and never become a public identifier,
a key derivation input, or model-context material.

Attenuation, proof-of-possession, revocation, and stateful enforcement are
requirements; no token format or deployed authentication system is selected. This documentation increment changes no kernel mechanism
or ABI. The [identity design](../../docs/architecture/identity-and-authority.md)
and [Passwordless Identity Log](../Development/2026-09-26%20Passwordless%20Identity%20Log.md)
record the implementation gaps, including exact task binding, trusted enrollment,
protected authenticators, durable recovery, and effect-time authorization.

## Bounded user mapping and physical-access planning proposal

Proposed [ADR 0041](../../docs/adr/0041-plan-bounded-user-mappings-and-physical-access.md)
selects an inert planning gate before replacing the live bootstrap mapper.
Eight user extents and eight exclusions have checked complete ranges, W^X,
exact executable membership, and independent data/user-table budgets. A separate
physical-window descriptor preserves distinct typed physical and kernel virtual
geometry. Neither output grants access, allocates leases, constructs pointers,
or activates a page-table root. The existing low-address reservation remains.

The 19 new tests, focused freestanding checks, and all 23 regression targets
passed. The [mapping log](../Development/2026-09-26%20User%20Mapping%20Plan%20Log.md)
records frozen-source verification and byte-identical runtime images.
This proves bounded planning and rejection, not allocation rollback or safe
activation. A later native proof must establish a valid privileged physical
access path and failure-atomic owned construction before low-address execution.

## Inactive owned user-mapping construction proposal

Proposed [ADR 0042](../../docs/adr/0042-prove-inactive-owned-user-mapping-construction.md)
selects retained, mutable ownership across allocation, table population,
validation, and retryable cleanup. Every acquired lease remains reachable until
verified release succeeds. Whole-arena table withdrawal precedes any data
unbind/release; once withdrawal begins, failures keep the stage unready and
retain quarantine ownership. Rejected preflight leaves the owner unchanged.
A temporary supervisor RW+NX high alias provides physical access under the
original bootstrap root, with explicit installation and removal ownership.

Focused host checks and all 24 regression targets passed, including memory
proofs at two RAM sizes under TCG and KVM. The
[construction log](../Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md)
records final outcomes, source verification, and matching image hashes.
The constructed user-only roots remain inactive; permanent kernel residency,
safe activation, global W^X, and general application launch remain open.

## Resident mapping activation proposal

Proposed [ADR 0043](../../docs/adr/0043-prove-resident-mapping-activation.md)
separates inactive construction, explicit resident geometry, and the unsafe
activation session that retains two owners and their physical alias. It selects
4 KiB supervisor mappings for a bounded kernel/stack island below 2 MiB and
requires exact descriptor/stack admission, hardware permission probes, root
restoration and resource reclamation. This feature-local CPL0 bridge is not a
permanent placement decision, a ring-3 launcher, or a global W^X claim. All 24
regression targets passed, including both memory sizes under TCG and KVM; normal,
dispatcher, and workspace images matched their baseline at that checkpoint.
See the [activation log](../Development/2026-09-26%20Resident%20Mapping%20Activation%20Log.md).

## Owned low-address user execution proposal

Proposed [ADR 0044](../../docs/adr/0044-prove-owned-low-address-user-execution.md)
connects retained mixed roots to real task/circle lifetimes in a removable
GPR-only ring-3 proof. Inert admission bounds declared initialized code and a
guarded writable stack; initial and resumed stack alignment are checked
separately. Exact private report/terminal predicates bind the live task, root,
phase, selectors, flags, complete exception frame, and instruction/stack bounds.
Two resumed reports precede B's denied supervisor read; A then demonstrates peer
progress, two reports, and exit. The private UD2 handshake does not become ABI.

Bootstrap restoration and root withdrawal precede mapping reclamation and
mapping/address-space child detachment. Task/circle destruction retires stale
generations, clears continuations, and restores all 28 frames/eight leases.
The actual long-mode hardware RSP/SS tail now supplies shared CPL0/CPL3/IST
snapshots. Normal, dispatcher, and workspace image hashes changed with that
shared correction; direct feature exclusion and runtime regressions passed.
All 24 validation targets passed, including the 96-MiB/320-MiB TCG/KVM memory
proofs, on 240 unchanged frozen source/configuration files. The
[user execution log](../Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records final results and image hashes. ADR 0044 remains Proposed. General
launch ownership, user-copy/fault policy, CPU-state expansion, permanent layout,
global W^X, and native artifact/foreign ABI work remain separate gates.

- [ADR 0051](../../docs/adr/0051-prove-userspace-elf-loading-against-native.md): Proposed, qualified userspace ELF loading comparison with the native application personality.
- [ADR 0052](../../docs/adr/0052-prove-linux-startup-and-record-device-syscalls.md): Proposed; bounded Linux startup and record-device syscall comparison qualified by 37 matrix jobs and two restorations; historical policy-placement gate followed by ADR 0053.

- [ADR 0053](../../docs/adr/0053-prove-a-supervised-userspace-syscall-veil.md): Proposed; independent userspace syscall policy, sealed blocked-caller transactions and dependency-fault containment implemented; bounded qualification complete. Fixed startup remains a controller fixture; ADR 0054 qualifies fixed two-caller membership, while general runtime services remain open.

- [ADR 0054](../../docs/adr/0054-prove-a-shared-multi-caller-userspace-veil.md): Proposed; shared two-caller userspace veil qualified by 41 qualification checks and two restorations; fixed serial membership, independent routes, caller survival and shared-loss containment.

- [ADR 0055](../../docs/adr/0055-prove-native-startup-and-private-userspace-arenas.md): Proposed; copied native startup, guarded private arenas and supplied document baselines implemented; bounded qualification complete.

- [ADR 0056](../../docs/adr/0056-prove-task-bound-native-document-open-and-save.md): Proposed; native RAM document open/save and exact snapshot acknowledgement implemented; bounded qualification complete.

- [ADR 0057](../../docs/adr/0057-prove-elf-loading-of-the-native-document-contract.md): Proposed; bounded ELF/native document comparison with verified zero-tail compaction implemented; qualification complete.

- [ADR 0058](../../docs/adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md): Proposed; bounded native editor replacement with retained service bytes and fresh authority is qualified through 50 checks and two development-image restorations.
- [ADR 0059](../../docs/adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md): Proposed; bounded adjacent native/ELF retained-service replacement with an independent oracle is qualified through 52 checks and two development-image restorations.
