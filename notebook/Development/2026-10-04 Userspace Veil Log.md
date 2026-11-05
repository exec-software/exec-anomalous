# Userspace Veil Log — 2026-10-04

## Outcome

The next ABI slice implements an independent, supervised ring-3 Linux syscall veil alongside the unchanged Linux editor producer and native editor service. The complete frozen-source qualification passes: 39 matrix jobs plus two restoration jobs, including TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with independent framebuffer captures.

This moves runtime syscall/device policy across the intended userspace boundary. It does not run full Antimatter/Electron, implement a general Linux process, introduce the native Reliquary artifact format, or qualify Ny application compilation/HAG. The fixed Linux startup stack remains a proof fixture, identified in the emitted evidence. Displacer remains a future desktop/theme integration reference; this slice does not change it.

Decision: [ADR 0053](../../docs/adr/0053-prove-a-supervised-userspace-syscall-veil.md). The preceding [Linux Application Log](2026-10-04%20Linux%20Application%20Log.md) and its 37+2-job evidence remain historical and unchanged.

## Implemented boundary

- `anomalous-veil-protocol` defines a versioned, copied 80-byte invocation and native mechanism results. Its Linux policy is behind `linux-policy`, enabled only in the standalone `anomalous-linux-veil` vessel.
- The new `veil_application_test` profile captures raw RAX and all six Linux argument registers, blocks the exact caller, and selects the independent native-entry veil. Syscall-number normalization, descriptor 3/4 routing, errno, poll fields, and return/yield/retire interpretation execute in the veil.
- The scheduler retains a move-only call authority branded to its original owner, exact task generations and reserved reply dispatch. It preserves the queued editor service. The veil blocks between calls and can be reawakened only by the original retained caller relation.
- Separate suspended-caller copies authenticate the running veil's root/TSS and the blocked caller's generation/frame/mapping owner. They inspect complete inactive PTE paths, permissions, frame leases and physical-alias spans without switching to the caller's CR3. Existing current-task copies retain their original requirements.
- One submit, consume or copyout may be staged. Staging changes no caller output or channel state. A reply validates the opaque request and stage token, action, saved frame and prepared scheduler return before effect publication. Consume performs the complete checked copy before infallibly clearing the channel. Yield/retire cannot carry an effect.
- Rejected completion preserves its stage. The native proof observes the blocked destination, caller frame, stage bytes, request and channel metadata before/after rejected completions, so a later valid copyout cannot hide an early write.
- A veil fault discards staging, revokes the call authority, contains both the veil and its waiting caller, and selects the editor service. It does not synthesize a Linux errno or restart either dependent task.

The kernel links only the protocol wire/mechanism portion. Artifact inspection rejects retained old kernel Linux adapter or userspace policy symbols in kernel text. The veil's embedded executable necessarily contains its own policy. Packaging checks that the ten preceding native/Linux/ELF blobs remain byte-identical.

## Resource and hardware contract

Three retained task owners use 75 frames: 48 data frames, including three four-page transition stacks, and 27 page-table frames. There are 15 active/bound leases. The concrete two-task aliases preserve the existing layout and behavior for prior proofs. The three-task runtime is constructed field by field in reserved BSS before publication.

Each task has a distinct root, transition stack and eager x87/SSE2 state. SCE is enabled only for the Linux producer. The native veil and editor service use the existing interrupt gate. Every SYSCALL uses the owned supervisor stack before storing user context. Return changes only the caller's RAX; the unchanged producer checks its registers, return frame and red zone. Cleanup restores original CR3, TSS, CPU controls and SYSCALL MSRs before withdrawing contexts and releasing mappings.

The first TCG run reported 24,093/81,437 free frames for 96/320 MiB guests, exactly 57,344 frames apart. Each scenario restores its initial accounting. No framebuffer access occurs under a userspace root.

## Compiled-code review

The exact reviewed kernel is `8e18d6010eb291cb8b81604fbb20c1ce65e198725192ea68db71898be461aff5`; the standalone veil is `f5921e5e48c0e50dbcab9ccfc6ed836402314785c5fa021ede010c444e31a554`. The final qualification kernel and veil match these retained binaries exactly.

The largest inspected supervisor transition path is 10,807 bytes of 16,384. The raw SYSCALL path is 2,967 bytes including normalized frame, alignment and return-slot allowance. The three-task bootstrap chain is approximately 36,168 bytes of 65,536; the native comparison in this image now uses 24,280 bytes after the retained-construction correction. The existing 16 KiB entry headroom guard remains unchanged. These are conservative emitted-chain estimates for admitted execution paths, not formal bounds for arbitrary fatal formatting paths.

Independent user-stack estimates are 1,064 bytes for the veil, 1,016 for the Linux producer including its 256-byte startup region, and 896 for the native client (848 for its service path), each within an 8 KiB user stack. There is no admitted recursion, dynamic stack adjustment or unresolved indirect call; local jump tables were inspected. The 176-byte kernel entry frame belongs to the separate supervisor stack.

The review verifies the trusted-stack switch before the first SYSCALL push, producer-only SCE selection, exact frame continuation and restoration of STAR/LSTAR/FMASK/EFER. The kernel resident end is `0x1c2000`, below the 2 MiB profile ceiling. The final receipt binds the full matrix to both reviewed hashes.

## Deterministic proof

The transcript requires exactly 26 records: the unchanged 21-record native reference, one veil readiness record, three scenario records and a pass record. Unexpected, missing, duplicated, reordered or changed evidence is rejected. The source-derived trace is:

| Scenario | Raw calls | Completed | Staged | Rejected native gates | Aborted stages | Published surfaces/inputs | Exits/faults |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Normal | 65 | 65 | 40 | 13 | 0 | 7 / 6 | 3 / 0 |
| Producer UD2 | 6 | 6 | 1 | 4 | 0 | 1 / 0 | 2 / 1 |
| Veil UD2 after staging | 6 | 5 | 1 | 12 | 1 | 0 / 0 | 1 / 2 |

Normal staging consists of seven submitted surfaces, six consumed inputs and 27 poll copyouts. The normal path rejects a wrong stage token and an invalid completion action for each effect kind, then completes the original operation. The veil-fault path rejects wrong request IDs, duplicate staging, invalid action and stale/wrong-token completion before its exact UD2. The native service runs with closed routes after both fault scenarios. The queued producer-fault surface is discarded; the staged veil-fault surface was never published and requires no queue discard.

The normal route preserves all seven source-state hashes, all transport counts and the final 384×24 public fixture patch. The whole 1024×768 framebuffer remains equal to the native reference, and neither fault scenario changes it. This is the bounded editor-state fixture using the host Antimatter reference, not the Electron application itself.

## Validation and OOM receipts

Focused host checks pass 479 kernel tests and eight protocol/policy tests. Strict target Clippy passes for the veil kernel and vessel. The first TCG smoke completes both debug-exit boots and the independent capture. Its capped service peaks at 1,496,002,560 bytes (about 1.39 GiB), with no cgroup or host OOM event.

The final matrix passes all 39 serialized jobs on 325 unchanged frozen non-Markdown source files, followed by two successful restoration jobs. It includes the host Antimatter reference, all required regressions, prior native/ELF/Linux/Ny proofs, TCG and actual KVM boots, and independent captures. Kernel tests pass 479 in the veil profile and 513 with all features; xtask passes 156 tests, and the protocol/policy crate passes eight. Whole TCG/KVM PPM files and the final kernel/image bytes match, and all ten preceding artifact blobs remain unchanged.

The qualified matrix peaks at 2,486,075,392 bytes (2.315 GiB), with minimum host available memory 21,521,383,424 bytes (20.043 GiB). Its cgroup reports 0 soft-limit events, zero hard-limit events and zero OOM events. Across every attempt, the maximum is 4,297,011,200 bytes; the first matrix hit the soft limit while mostly retaining file cache, but never exhausted memory. All host OOM counters remain zero. The KVM wrapper records 28 actual KVM invocations, including the three veil boots/capture runs.

Both normal and workspace development images are restored. The normal image still matches the preceding slice; the workspace image is requalified after shared owner changes. Retained SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| Normal image | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| Workspace image | `8bc1f773cbf7a8b7de54c957321c53e6f1fcb03f5f9d74848a9f8ecb847909ab` |
| Veil proof image | `aff0ca324cdf1ab36c9bc45a7e3975a694142b5499f0d0ba504b2db06595e106` |
| Veil proof kernel | `8e18d6010eb291cb8b81604fbb20c1ce65e198725192ea68db71898be461aff5` |
| Veil vessel | `f5921e5e48c0e50dbcab9ccfc6ed836402314785c5fa021ede010c444e31a554` |

Fresh evidence is retained under an external validation directory (not distributed). Previous Linux/ELF/native/Ny evidence directories are not overwritten. Every build/QEMU job runs with `CARGO_BUILD_JOBS=2`, Make `-j1`, `MemoryHigh=4G`, `MemoryMax=6G`, and `MemorySwapMax=512M`. The runner samples memory every two seconds and terminates its own process group if host available memory falls below 4 GiB or a new OOM is observed.

## Regression caught and corrected during qualification

The first matrix passed `make check`, the host reference and the new veil proof, then the preceding Linux profile stopped at `owned_bootstrap_state`. Emitted inspection traced this to a 480-byte increase in the two-task `run_pair` frame after the generic owner refactor. That path had only a few hundred bytes of margin above the existing 16 KiB entry headroom requirement.

The correction outlines `initialize_pair` and constructs each owner directly in retained BSS before preparation, matching the three-task lifetime pattern. It initializes every field before forming the full runtime reference, retains partial ownership behind the storage latch and keeps the existing stack sizes and guard unchanged. The native-only construction path remains separate. Emitted inspection of the fixed older Linux profile reduces its bootstrap chain from 49,904 to 25,792 bytes, a 24,112-byte reduction. The three-task image now has 29,368 bytes left on its maximum inspected bootstrap path. The source set was frozen again, and the corrected Linux and veil profiles both passed their two boots and capture at the start of a fresh complete 39-job matrix.

The failed matrix's runner/build logs, cgroup receipt, source hashes and failing Linux kernel are retained. It was a guest stack guard rejection, not host memory exhaustion; all host/cgroup OOM counters stayed at zero.

## Next gate

Generalize the qualified one-caller veil into a supervised service with explicit multi-caller lifecycle and authority, then expand the native application facilities that Antimatter needs: process/runtime startup, memory/thread/TLS support, event delivery, filesystem access and richer window/content presentation. The next capability should be chosen against the native editor path and the host Linux Antimatter behavior, retaining native/Linux comparisons at each boundary. Windows and macOS remain independent later personalities.
