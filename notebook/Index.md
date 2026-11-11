---
title: "Anomalous — Project Notebook"
date: 2026-11-11
tags: [index, anomalous, operating-system, rust, display, desktop]
status: active
domain: operating-system
---

# Anomalous — Project Notebook

This is the working memory for Anomalous. It records what happened, what was
observed, what remains uncertain, and why the next experiment is worth doing.
Formal specifications and architecture decisions remain in [`docs/`](../docs/);
the notebook may contain incomplete ideas and must label them accordingly.

## Start here

| Note | Purpose |
| --- | --- |
| [Current State](Overview/Current%20State.md) | Verified capabilities, known gaps, and the current development target |
| [Native Program Console Log](Development/2026-11-11%20Native%20Program%20Console%20Log.md) | Experimental ANPG console transport with live authority, complete copied text, checked replies and hostile-request rejection; all 27 final jobs and TCG/KVM proofs passed, with no edition execution |
| [Owned Loader Sources Log](Development/2026-10-07%20Owned%20Loader%20Sources%20Log.md) | Copied and sealed native source ownership across producer retirement; TCG/KVM/VirtualBox qualification and guarded regressions |
| [ELF Relaunch Log](Development/2026-10-04%20ELF%20Relaunch%20Log.md) | Bounded userspace ELF retained-service replacement comparison against the native baseline; all 52 qualification checks and two restorations passed |
| [Native Relaunch Log](Development/2026-10-04%20Native%20Relaunch%20Log.md) | Native editor replacement with a retained RAM document service and fresh authority; all 50 qualification checks and two restorations passed |
| [ELF Document Log](Development/2026-10-04%20ELF%20Document%20Log.md) | Same bounded document contract through native/ELF spans with verified zero-tail compaction; qualification complete |
| [Native Document Log](Development/2026-10-04%20Native%20Document%20Log.md) | Native RAM open/save/close/reopen, exact snapshot acknowledgements and uncertain outcomes; bounded qualification complete |
| [Native Runtime Log](Development/2026-10-04%20Native%20Runtime%20Log.md) | Copied native startup, guarded private arenas and arena-backed editor state; implementation complete, bounded qualification complete |
| [Userspace Veil Log](Development/2026-10-04%20Userspace%20Veil%20Log.md) | Independent three-task syscall-policy boundary and staged dependency-fault containment; bounded qualification complete |
| [Linux Application Log](Development/2026-10-04%20Linux%20Application%20Log.md) | Qualified Linux startup/SYSCALL and bounded record-device comparison; 37 matrix jobs plus two restorations |
| [ELF Application Log](Development/2026-10-03%20ELF%20Application%20Log.md) | Qualified userspace ELF/native loading comparison; 35 matrix jobs and two restoration jobs passed |
| [Shared Veil Log](Development/2026-10-04%20Shared%20Veil%20Log.md) | Two callers share a supervised userspace veil with independent routes, caller survival and shared-fault revocation; all 41 qualification checks and two restorations passed, no OOM |
| [Native Application Log](Development/2026-10-03%20Native%20Application%20Log.md) | Independent bounded Rust editor/service tasks, real Antimatter reference behavior, copied surface/input and deferred presentation; all 33 matrix jobs passed on 291 unchanged frozen files |
| [Ny Object Log](Development/2026-10-03%20Ny%20Object%20Log.md) | Pinned Ny scalar object, inspected startup/runtime shim, initialized-data and timer-return proof, and failure/peer contrast; all 30 targets passed on 272 unchanged frozen files |
| [Application Profile Log](Development/2026-10-03%20Application%20Profile%20Log.md) | Internal eager x87/SSE2 state, bounded owner-checked copies and selected user faults through last-current cleanup; all 28 targets passed on 265 unchanged frozen files, including TCG/AMD KVM |
| [Owned Sustained Dispatch Log](Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md) | Two retained launches through 64 preemptions and 16 survivor ticks, admission rollback and stack cleanup retries; all 26 targets qualified across documented phases, with TCG/KVM evidence |
| [Prepared Launch Ownership Log](Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md) | Retained preparation, explicit task publication, cancellation and cleanup retries toward the native Antimatter ABI workload |
| [Native Antimatter Direction Log](Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md) | Official Antimatter/Ny/HAG direction, sibling source audit, and proposed native editor acceptance after kernel gates |
| [Development Environment Recovery Log](Development/2026-10-03%20Development%20Environment%20Recovery%20Log.md) | Replacement host setup, QEMU corrections, all 24 passed targets, memory limits, and workspace launch |
| [Owned User Execution Log](Development/2026-09-27%20Owned%20User%20Execution%20Log.md) | Two real low-address ring-3 tasks, exact resumed-stack/terminal proofs, complete restoration, and all 24 passed validation targets including TCG/KVM |
| [User Mapping Plan Log](Development/2026-09-26%20User%20Mapping%20Plan%20Log.md) | Inert multi-region geometry, checked physical/kernel address domains, 19 new tests, and 23 passed regression targets |
| [Owned Mapping Construction Log](Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md) | Real inactive user trees, a temporary supervisor physical alias, retained failure ownership, cleanup retry, and 24 passed regression targets |
| [Resident Mapping Activation Log](Development/2026-09-26%20Resident%20Mapping%20Activation%20Log.md) | Two active low-user roots, explicit supervisor residency, exact CPL0 permission faults, physical-alias collision proof, complete restoration, and 24 passed regression targets |
| [Passwordless Identity Log](Development/2026-09-26%20Passwordless%20Identity%20Log.md) | Proposed entity, authenticator, authority, administration, recovery, and future biometric boundaries |
| [Identity and Authority](../docs/architecture/identity-and-authority.md) | Proposed modular passwordless architecture; implemented kernel identity remains distinct from authentication |
| [Sustained Native Dispatch Log](Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md) | Validated sustained-dispatch proof, final TCG/KVM and full-matrix evidence, VirtualBox limit, and native ABI readiness correction |
| [Execution Readiness](../docs/architecture/execution-readiness.md) | Source-backed native ABI/artifact audit and launch, Ny/Antimatter, Reliquary, Linux, and macOS gates |
| [Window Content Surfaces Log](Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md) | Bounded caller-rendered window content, granted ownership, Graphics Lab, and final native/regression evidence |
| [Mouse Responsiveness Log](Development/2026-09-26%20Mouse%20Responsiveness%20Log.md) | Measured pointer improvement, bounded batching, exact compositor/transfer optimizations, and final native/regression evidence |
| [Desktop Click Lifecycle Log](Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md) | Cancellable release activation, canonical pressed feedback, and the checkpoint before further kernel work |
| [Appearance App Log](Development/2026-09-26%20Appearance%20App%20Log.md) | Built-in settings, canonical choice selection, recoverable policy rejection, and final native/full-matrix evidence |
| [Clay Frame Assets Log](Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md) | Clay-inspired official pair, original vector controls, canonical pressed state, and validation evidence |
| [Native Themes and Pointer Log](Development/2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md) | First official themes, pointer window shell, and repeatable native sessions |
| [Native Workspace Development Log](Development/2026-09-26%20Native%20Workspace%20Development%20Log.md) | Persistent native window owner, canonical human/machine commands, software composition, and measured development evidence |
| [Interactive Desktop Development Log](Development/2026-09-26%20Interactive%20Desktop%20Development%20Log.md) | Native-rendered launcher, returning bootstrap shell, and interactive VM evidence |
| [Decision Register](Decisions/Decision%20Register.md) | Accepted constraints and decisions that are still open |
| [Foundation Log](Development/2026-09-08%20Foundation%20Log.md) | Chronological record of the seed milestone and current direction |
| [Bootstrap Shell Log](Development/2026-09-08%20Bootstrap%20Shell%20Log.md) | Implementation and VM evidence for the first interactive vertical slice |
| [Diagnostic Fault Spine Log](Development/2026-09-08%20Diagnostic%20Fault%20Spine%20Log.md) | GDT, TSS, exception-path, QEMU, and VirtualBox evidence |
| [First Circle Log](Development/2026-09-09%20First%20Circle%20Log.md) | QEMU, VirtualBox, teardown, exclusion, and acceptance evidence for the fixed ring-3 experiment |
| [Memory Ownership Log](Development/2026-09-09%20Memory%20Ownership%20Log.md) | Recovery, authority review, QEMU/VirtualBox evidence, and acceptance of bounded frame/mapping ownership |
| [Interrupt, Time, and Input Log](Development/2026-09-09%20Interrupt%20Time%20and%20Input%20Log.md) | Implementation, QEMU/VirtualBox evidence, and acceptance of returning IRQs, monotonic ticks, and private physical input |
| [Task, Circle, IPC, and Sigil Log](Development/2026-09-09%20Task%20Circle%20IPC%20and%20Sigil%20Log.md) | Implementation, review, QEMU/VirtualBox evidence, and acceptance of the fixed ring-3 authority and copied-message proof |
| [Bounded Scheduling and Fault Containment Log](Development/2026-09-09%20Bounded%20Scheduling%20and%20Fault%20Containment%20Log.md) | Implementation, review, QEMU/VirtualBox evidence, and acceptance of fixed two-task preemption and user-fault containment |
| [Eidolon and Ny Integration Log](Development/2026-09-09%20Eidolon%20and%20Ny%20Integration%20Log.md) | Ny SDK, codec-oracle, interpreted NIL, and cross-route integration evidence |
| [Bounded Vessel Supervision and Restart Log](Development/2026-09-09%20Bounded%20Vessel%20Supervision%20and%20Restart%20Log.md) | Accepted blocked-task, supervision-observation, safe checkpoint, and destructive replacement proof |
| [Supervised Vessel Rite ABI Log](Development/2026-09-13%20Supervised%20Vessel%20Rite%20ABI%20Log.md) | First shell/display-manager vessel kernel-boundary notes and evidence |
| [Supervised Shell Vessel Composition Log](Development/2026-09-13%20Supervised%20Shell%20Vessel%20Composition%20Log.md) | Accepted fixed composition of scheduling, restart supervision, native rites, and human/machine projections |
| [Private Shell Session Ingress Log](Development/2026-09-13%20Private%20Shell%20Session%20Ingress%20Log.md) | Accepted host-only bounded private semantic input and lost-wake-safe session proof |
| [Native Session Input and Exact Wake Log](Development/2026-09-13%20Native%20Session%20Input%20and%20Exact%20Wake%20Log.md) | Accepted versioned native reply, owned pending read, and exact scheduler-wake proof with QEMU evidence |
| [Authenticated-Source Session Routing Log](Development/2026-09-19%20Authenticated%20Source%20Session%20Routing%20Log.md) | Accepted two-route human-focus and machine-explicit routing proof with private incarnation and per-source sequencing |
| [User-Space Display Presentation Log](Development/2026-09-19%20User-Space%20Display%20Presentation%20Log.md) | Accepted bounded ring-3 RGB presentation path, exact QEMU frame evidence, and interactive review command |
| [Canonical Semantic Desktop Scene Log](Development/2026-09-19%20Canonical%20Semantic%20Desktop%20Scene%20Log.md) | Accepted eight-node canonical scene contract, paired human/machine projections, and exhaustive bounded state proof |
| [Supervised Semantic Renderer Composition Log](Development/2026-09-19%20Supervised%20Semantic%20Renderer%20Composition%20Log.md) | Accepted deterministic scene capsule, supervised two-generation ring-3 renderer, and verified whole-frame publication |
| [Transactional Semantic Surface Publication Log](Development/2026-09-19%20Transactional%20Semantic%20Surface%20Publication%20Log.md) | Accepted host-only one-successor transaction service, strict owner-returning backpressure, fresh renderer retry, and presentation-gated paired semantic publication |
| [Native Semantic Surface Transport Log](Development/2026-09-19%20Native%20Semantic%20Surface%20Transport%20Log.md) | Accepted explicit 528-byte wire body, copied one-slot native delivery, privacy/fault evidence, and exact QEMU teardown |
| [Native Semantic Presentation Composition Log](Development/2026-09-20%20Native%20Semantic%20Presentation%20Composition%20Log.md) | Accepted live-transaction transport, isolated rendering, private staging, exact pixels, and presentation-gated semantic publication |
| [Shell Language Questions](Research/Shell%20Language%20Questions.md) | Security, syntax, and human/machine questions to resolve before implementation |
| [Presentation Recovery Coordination Log](Development/2026-09-26%20Presentation%20Recovery%20Coordination%20Log.md) | Host recovery prerequisite, observation gate, fresh-presenter redraw and native deployment constraints |

## Organization

| Directory | Contents |
| --- | --- |
| `Overview/` | Current project state and near-term direction |
| `Decisions/` | Working decision register; accepted platform decisions live in `docs/adr/` |
| `Development/` | Dated engineering session logs and bring-up evidence |
| `Research/` | Design spaces, hypotheses, comparisons, and open questions |
| `Templates/` | Repeatable note formats for future work |

## Notebook discipline

- Date meaningful updates and preserve the chronological log.
- Mark statements as **observed**, **decided**, **proposed**, or **unknown**.
- Record the command, artifact, or trace behind an observed result.
- Put privilege boundaries, stable formats, and platform abstractions through
  the repository ADR process before treating them as settled.
- Keep secrets, credentials, private user data, and unredacted model context out
  of the notebook.
- Prefer a falsifiable question and an explicit success condition over a vague
  task description.

## Status at a glance

> **Phase:** Kernel foundations are in progress. The fixed first-circle,
> bounded memory, interrupt/input, task/sigil/IPC, scheduling, one-restart
> supervision, native x86-64 rite-gate, supervised shell-vessel composition, and
> host-only private shell-session ingress slices are accepted. The native
> session-input and exact-wake bridge is accepted in ADR 0022, and the bounded
> host authenticated-source router is accepted in ADR 0023. ADR 0024 accepts
> the first bounded ring-3 display-presentation path. ADR 0025 accepts the first
> bounded canonical semantic desktop scene contract. ADR 0026 accepts its next
> removable supervised renderer composition.
> [ADR 0027](../docs/adr/0027-prove-bounded-transactional-semantic-surface-publication.md)
> accepts the separate host-only transactional surface service that retains one
> private successor through rendering and publishes its human, accessibility,
> and machine semantics together only after an exact trusted-host presentation
> receipt. ADR 0028 accepts the separate removable native transport that copies
> one explicit redacted body from a narrowly granted source through a kernel-
> owned slot to a separately granted consumer. ADR 0029 accepts their bounded
> composition for one successful attempt: a live retained successor drives the
> isolated renderer and verified framebuffer before its paired semantics
> publish exactly at revision/sequence 2/2. Canticle remains a proposed
> user-space design.
>
> **Observed:** At display implementation checkpoint `b6546cc`, `make check`
> passed 22 `anomalous-abi` unit tests plus 6 compile-fail documentation tests;
> 51
> `anomalous-session` unit tests plus 56 compile-fail documentation tests;
> 24 `anomalous-session-router` unit tests plus 46 compile-fail documentation
> tests; 307 focused display-feature kernel tests; and 46 `xtask` tests.
> Formatting, script syntax, warning-denied host/x86-64 Clippy, an explicit
> all-feature x86-64 ELF link, every established QEMU smoke target, and
> normal-image exclusion passed. The display proof accepted exactly 16 records
> and 772 calls, preserved the entire framebuffer for three rejected requests,
> restored all tracked ownership, and matched all 786,432 captured pixels.
> VirtualBox was not run or required for the removable QEMU mechanism proof.
>
> ADR 0025 acceptance checkpoint `2857d8c` adds a dependency-free, `no_std`,
> allocation-free, unsafe-free scene proof. Eighteen unit tests and 129
> ownership/privacy compile-fail tests passed; the implementation exhausts all
> 96 declared states, derives exact 25/21-command human plans and matching
> eight-node accessible/machine projections. The kernel dependency tree
> confirms it remains absent from the normal image. The complete host check,
> warning-denied Clippy, all established QEMU lanes, and the unchanged
> `normal-exclusion` regression passed.
>
> ADR 0026 accepts a 6,469-byte, two-page fixed renderer load image and its
> complete two-generation native lifecycle: one real PIT, 384 staged rows, exact `#UD`
> containment and withdrawal, then a fresh 768-row terminal generation and one
> full-frame commit. Two protected-source builds were byte-identical, and an
> independent oracle matched all 786,432 captured pixels. Exact resource
> restoration and normal-image exclusion include the private sentinels and
> renderer glyph fingerprint. `make check` passed with 329 all-feature kernel,
> 56 `xtask`, 8 renderer-base, and 9 scene-conformance tests; the full
> established non-VirtualBox QEMU matrix passed. VirtualBox was not used.
>
> ADR 0027 implementation checkpoint `2076eba` adds the host-only,
> `no_std`, allocation-free, unsafe-free `anomalous-desktop-service` proof and
> ADR 0025's borrowed prepared-preview/candidate-recovery path. Focused evidence
> passed 19 scene unit tests, 136 scene documentation/compile-fail tests, 14
> service unit tests, 74 service documentation/compile-fail tests, and
> warning-denied Clippy for both crates. The service evidence covers all 64
> valid constructors, all 32 invalid constructors, all 32 invalid successor
> states, and all 4,096 valid state transitions; the committed typed observation
> stays unchanged through queued, leased, ready, and presented phases and
> changes only after publication. `make check` passed, including 329 all-feature
> kernel tests, 56 `xtask` tests, renderer base/conformance tests, artifact and
> freestanding-link checks, normal-image exclusion, formatting, scripts, and
> warning-denied host/x86-64 Clippy. The full established QEMU regression matrix
> and a final `normal-exclusion` run passed, and `cargo tree -p kernel` excludes
> the service crate. VirtualBox was not run. The service itself did not execute
> in QEMU; those emulator results are unchanged native regression evidence, and
> its trusted host receipt does not verify renderer integrity, pixels, or
> scanout.
>
> ADR 0028 acceptance checkpoint `228a168` adds one 528-byte explicit little-
> endian body and disjoint submit/read Rite paths. The packaged source and
> consumer flat images are 2,384 bytes/one page and 5,094 bytes/two pages; their
> ELF files are 1,107,792 and 1,185,928 bytes. The strict QEMU proof completed
> 11 calls, 9 copies totaling 2,176 bytes, one submit, one delivery, one
> backpressure result, two authority denials, two malformed results, two exits,
> and two restored continuations. Every source and destination copy offset was
> fault-injected, protected-source variants were byte-identical and absent from
> artifacts/transcript, header-last and stale clearing held, the source retired
> before delivery, and teardown restored exact accounting. The complete host
> and non-VirtualBox QEMU matrix, dedicated smoke, artifact/link/precedence
> checks, warning-denied Clippy, and normal-image exclusion passed.
>
> ADR 0029 implementation checkpoint `2f453e7` retains one live ADR 0027
> successor while its exact ordinal-61 ADR 0028 body crosses the copied native
> endpoint into a separately linked ring-3 renderer. The accepted QEMU run
> completed 777 Rite calls, 775 body copies totaling 2,392,160 bytes, 768
> ascending rows into one private 576-frame stage, one framebuffer commit, and
> exact restoration of all 65,095 free frames. Kernel readback and an
> independent host oracle verified the frame; the latter matched all 786,432
> RGB pixels before the retained private controls published revision/sequence
> 2/2. The 2,384-byte one-page source and 10,352-byte three-page renderer hashes
> are recorded in the linked development log. This is one successful attempt;
> early-exit refusal has structural/component evidence rather than a negative
> QEMU run, the declared baseline is not a proven revision-1 visual, and the
> framebuffer-to-host-publication crash gap remains. VirtualBox was not used.
>
> **Implemented and validated:** Proposed ADR 0032 adds a separate native
> workspace path
> around a portable window model and software compositor. Four public demo
> windows and two workspaces exercise shared granted human/machine commands,
> focus, lifecycle, tiling, and visual settings. Native smoke passed 21 exact
> full frames and five termination paths with restored resources and working
> shell fallback. The full 20-target validation matrix passed; the workspace
> development log records the evidence. At that checkpoint, interactive testing
> exited the workspace cleanly and QEMU remained at the bootstrap prompt.
> VirtualBox startup was blocked by the host KVM/AMD-V conflict. Accepted proof
> history above is unchanged.
>
> **Implemented and validated:** Proposed ADR 0033 adds official Light/Dark themes,
> independent appearance components, mouse window controls and task panel,
> maximize/restore, and a fresh `desktop` return from the shell. All 20 validation
> targets passed; native smoke verified 21 keyboard and 124 pointer frames across
> two sessions. A fresh QEMU was opened for hands-on testing. The theme and pointer
> log records the evidence, including the continuing VirtualBox host limitation.
>
> **Implemented and validated:** Proposed ADR 0034 adds original compiled frame
> controls, Clay-inspired Dark and interpreted Light, independently validated
> frame components, blended shadows, and canonical pressed state. All 20
> validation targets passed; the final compositor suite passed 31 tests. A fresh
> validated QEMU reached its complete first native frame. Generated SVG sheets
> remain design references rather than native pixel goldens. The [Clay Frame Assets Log](Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md)
> tracks this evidence separately from the previous full validation pass.
>
> **Implemented and validated:** Proposed ADR 0035 adds the built-in Appearance
> app, sixteen explicit preferences/actions, canonical keyboard selection, and
> recoverable bounded policy rejection. All 20 validation targets passed;
> native smoke verified 21 keyboard and 337 pointer/Appearance frames across
> two sessions. The final `make check` was repeated after the corner-override
> correction. The [Appearance App Log](Development/2026-09-26%20Appearance%20App%20Log.md)
> records the final counts, target durations, image, and interactive session.
>
> **Desktop interaction checkpoint:** Proposed ADR 0036 adds cancellable
> release-to-activate controls with canonical armed feedback. All 20 validation
> targets passed; native smoke verified 21 keyboard and 410 pointer/Appearance
> frames, including 73 new release-gesture frames. See the [Desktop Click Lifecycle Log](Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md).
> The subsequent mouse follow-up is recorded separately below.
>
> **Mouse fix implemented and validated:** Composition and bounded frame transfer avoid
> redundant work, and graphical Linux QEMU prefers KVM with TCG fallback.
> Proposed [ADR 0037](../docs/adr/0037-batch-ready-desktop-pointer-motion.md)
> batches at most 16 ready continuous motion packets with complete raw
> validation, ordered clamping, and separate semantic boundaries. No wait is
> added; one canonical update still needs full-frame acknowledgement.
> Final native timings and five sustained-input frame comparisons passed;
> all 20 validation targets and an additional complete KVM workspace smoke passed. Evidence is in the [Mouse Responsiveness Log](Development/2026-09-26%20Mouse%20Responsiveness%20Log.md).
>
> **Native graphics implemented and validated:** Proposed
> [ADR 0038](../docs/adr/0038-add-native-window-content-surfaces.md) adds immutable
> RGB8 window content, caller-owned raster drawing, a granted window-specific
> presentation owner, and complete-frame publication. Graphics Lab exercises
> the path inside the existing trusted workspace vessel. All 20 validation
> targets and an additional complete KVM workspace smoke passed, including
> 23 exact graphics frames per workspace run. Native timing and an exact
> sustained-input capture also passed. Evidence is in the
> [Window Content Surfaces Log](Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md).
> Linux-first TRI-ABI remains the intended direction; no foreign ABI or display
> protocol is implemented by this native graphics increment.
>
> **Sustained dispatch implemented and validated:** Proposed
> [ADR 0039](../docs/adr/0039-prove-bounded-sustained-native-dispatch.md) adds a
> separate sustained-dispatch proof: 64 real PIT switches, one exact known fault,
> and 16 sole-runnable timer ticks with measurable progress and no artificial
> dispatch-sequence advance. Final TCG and KVM runs passed the 14-record
> transcript with 80 total ticks, 66 dispatches, and all 65,112 free frames
> restored. All 23 validation targets passed; 228 frozen source/configuration
> files stayed unchanged. An isolated VirtualBox attempt was blocked by host
> KVM ownership of AMD-V and cleaned up. The
> [Sustained Native Dispatch Log](Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md)
> records the new increment. The native format is not done: experimental rites
> exist, but canonical invocation encoding, a general versioned launch
> contract, and Reliquary framing remained open at that checkpoint. Its planned
> Linux ELF/syscall follow-up is now bounded by ADRs 0051/0052; the independent
> Mach-O/Darwin veil remains open.
>
> **Owned-user checkpoint:** Proposed
> [ADR 0044](../docs/adr/0044-prove-owned-low-address-user-execution.md) executes
> two real GPR-only ring-3 tasks over owned low-address roots. Four exact private
> reports resume at noninitial stack pointers; B's denied supervisor read faults
> only B, then A continues to exit. All 28 frames, eight leases, task/circle
> identities, and transition state are restored. Shared exception diagnostics
> now use the actual hardware-saved RSP/SS. All 24 validation targets passed,
> including both RAM sizes under TCG and KVM, on 240 unchanged frozen source
> files. The [Owned User Execution Log](Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
> records final results and measured runtime-image changes. This does not define a syscall ABI, loader, general user-copy, or permanent
> kernel layout.
>
> **Preceding kernel increment:** Proposed
> [ADR 0047](../docs/adr/0047-prove-sustained-dispatch-over-retained-launches.md)
> composes two retained prepared launches with 64 timer preemptions, an exact
> UD2 fault, and 16 survivor ticks. Both owners remain live until both tasks are
> terminal; partial admission withdrawal and two failed stack clears preserve
> cleanup ownership. Focused tests, 96-MiB/320-MiB TCG/KVM proofs, and all 26
> regression targets qualified across documented phases. The final 258-file
> freeze remained unchanged after a narrow display-capture harness correction.
> The
> [Owned Sustained Dispatch Log](Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
> records that checkpoint's evidence.
>
> **Preceding kernel increment:** Proposed
> [ADR 0048](../docs/adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
> adds eager x87/SSE2 ownership, bounded current-task copies and selected user
> faults through last-current cleanup. All 28 targets passed on 265 unchanged
> frozen files, including both RAM sizes under TCG and actual AMD-host KVM.
> The [Application Profile Log](Development/2026-10-03%20Application%20Profile%20Log.md)
> records final evidence and limits. TLS, AVX/XSAVE and full untrusted-program
> admission remain open; Antimatter is the first intended actual application ABI test.
>
> **Preceding increment:** Proposed
> [ADR 0049](../docs/adr/0049-qualify-a-bounded-freestanding-ny-object.md)
> qualifies one pinned Ny object and inspected startup shim, with real generated
> integer/f64 work, copied initialized data and failure/peer contrast. All 30 targets
> passed on 272 unchanged frozen files, including two-size TCG/AMD KVM proofs.
> The [Ny Object Log](Development/2026-10-03%20Ny%20Object%20Log.md) records final evidence.
> Task-bound surface/input and a native Antimatter frontend follow; the hosted Ny
> runtime, general executable admission and workspace persistence remain open.
>
> **Completed native application checkpoint:** Proposed
> [ADR 0050](../docs/adr/0050-prove-a-bounded-native-application-contract.md)
> implements independent Rust editor/service tasks, copied surface/input, a
> seven-state real Antimatter reference trace and deferred final presentation.
> All 33 matrix jobs passed on 291 unchanged frozen files, including two-size
> TCG/KVM and capture proofs. Final evidence is in the
> [Native Application Log](Development/2026-10-03%20Native%20Application%20Log.md).
> ADRs 0051/0052 subsequently qualify bounded ELF and Linux entry/device
> comparisons. This is no full editor port.
>
> **Completed Linux checkpoint:** Proposed
> [ADR 0052](../docs/adr/0052-prove-linux-startup-and-record-device-syscalls.md)
> qualifies the Linux producer/native service comparison with 37 matrix jobs
> and two restorations on 313 unchanged frozen files. TCG/KVM two-size boots
> and captures match; its next userspace-policy gate is implemented by ADR 0053.
> The [Linux Application Log](Development/2026-10-04%20Linux%20Application%20Log.md)
> records the fixed device subset and limits.
>
> **Completed veil checkpoint:** Proposed [ADR 0053](../docs/adr/0053-prove-a-supervised-userspace-syscall-veil.md)
> implements a third userspace policy task, sealed blocked-caller transactions,
> inert effects and dependency-fault containment. All 39 matrix jobs and two restorations passed; evidence is
> in the [Userspace Veil Log](Development/2026-10-04%20Userspace%20Veil%20Log.md). Fixed startup
> remains a controller fixture; ADR 0054 subsequently qualified fixed two-caller
> supervision. General runtime/service capabilities remain separately gated.
>
> **Completed startup checkpoint:** Proposed
> [ADR 0055](../docs/adr/0055-prove-native-startup-and-private-userspace-arenas.md) implements copied native startup
> and private arena allocations for the bounded editor client. All 43 checks
> and two image restorations passed; receipts are in the [Native Runtime Log](Development/2026-10-04%20Native%20Runtime%20Log.md).
> **Completed document checkpoint:** Proposed [ADR 0056](../docs/adr/0056-prove-task-bound-native-document-open-and-save.md)
> implements native RAM open/save/close/reopen with exact snapshot replies and
> unknown-outcome handling. Bounded native qualification passed; evidence is in
> the [Native Document Log](Development/2026-10-04%20Native%20Document%20Log.md). No saved
> data survives this RAM service's destruction; full Antimatter/Electron is open.
>
> **Completed document comparison:** Proposed [ADR 0057](../docs/adr/0057-prove-elf-loading-of-the-native-document-contract.md)
> qualifies the same bounded contract through ELF without widening parser or
> loader bounds. The [ELF Document Log](Development/2026-10-04%20ELF%20Document%20Log.md)
> records the completed qualification.
>
> **Current native slice:** Proposed [ADR 0058](../docs/adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md)
> adds editor replacement with the same RAM service retained. Independent packet
> and surface checkpoints distinguish discarded saves from committed saves with
> lost replies. All 50 qualification checks and two development-image restorations
> passed; the [Native Relaunch Log](Development/2026-10-04%20Native%20Relaunch%20Log.md)
> records completed bounded qualification.
> Proposed [ADR 0059](../docs/adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md)
> qualifies the matching bounded userspace ELF replacement comparison with an
> independent 222-record oracle. All 52 qualification checks and two restorations
> passed; the [ELF Relaunch Log](Development/2026-10-04%20ELF%20Relaunch%20Log.md)
> records the completed evidence.
> Scoped filesystem persistence with explicit durability semantics follows.
>
> **Open:** Independent application producers, private-content policy, an authenticated
> external agent IPC API, service-loss recovery, text input, durable
> configuration, scanout/page-flip/GPU completion, and a normal-image desktop.
> A stable graphics ABI, production display manager, Canticle, credential
> authentication, production scheduling and supervision, general vessel launch,
> and a real user-space shell runtime also remain open.
>
> **Blocking:** no project blocker; many required kernel and user-space
> facilities are not yet implemented.

- [2026-10-03 ELF Application Log](Development/2026-10-03%20ELF%20Application%20Log.md): completed userspace loader and native/ELF comparison qualification.
- [2026-10-04 Linux Application Log](Development/2026-10-04%20Linux%20Application%20Log.md): qualified bounded Linux startup/SYSCALL and record-device comparison; historical gate followed by ADR 0053.
- [2026-10-04 Userspace Veil Log](Development/2026-10-04%20Userspace%20Veil%20Log.md): implemented userspace syscall policy and dependency-fault containment; bounded qualification complete.
