# ADR 0053: Prove a supervised userspace syscall veil

- Status: Proposed; bounded proof qualified by 39 matrix jobs and two restorations
- Date: 2026-10-04
- Supersedes: the next runtime-policy gate in ADR 0052, not its historical evidence
- Related: ADR 0008, ADR 0048, ADR 0050, ADR 0051, ADR 0052

## Context

ADR 0052 qualified an unchanged Linux-convention editor producer against the native editor service, but syscall numbers, descriptor routing, errno and poll policy still executed in a feature-private kernel adapter. Anomalous's compatibility direction requires that policy to execute in a supervised userspace vessel. The native contract remains the reference for later Linux, Windows and macOS comparisons. Full Antimatter/Electron, Ny application compilation, HAG and a general Linux process environment remain separate gates.

## Decision

Add the opt-in `veil_application_test` profile with three independently retained ring-3 tasks: the existing Linux producer, the existing native editor service and a new native-entry Linux veil. Preserve both original application binaries. The veil interprets the raw syscall number, including signed low-32-bit normalization, descriptors, lengths, errno, poll records and return/yield/retire decisions. Kernel dependencies include only the protocol's wire and native mechanism definitions; Linux policy is built only into the veil vessel. The preceding kernel-policy Linux profile remains independently testable as historical comparison evidence.

The kernel captures RAX and six argument registers without interpreting them and blocks the exact caller. A sealed scheduler authority binds the original scheduler owner, exact task generations, caller, veil and reserved return dispatch. The native service remains queued. The veil blocks between replies; only the same retained caller relation can wake it again. Timer interrupts resume the currently selected task. SCE is enabled only for the Linux producer; each task has its own root, transition stack and eager x87/SSE2 image.

A copied 80-byte, versioned little-endian request exposes a globally nonwrapping opaque request number and raw arguments. It exposes no task identity, root, physical address or saved frame. Every veil mechanism authenticates the actual current veil and sealed blocked caller. A separate checked-copy path verifies the inactive caller's retained mappings, permissions, leases and complete physical-alias spans; it never weakens the existing current-task copy path or selects the caller's CR3 to copy.

Native mechanisms provide bounded route snapshots, copied caller reads, one staged submit/consume/copyout effect, and completion. Staging is inert. Completion validates request, stage correlation, action, owned return frame and scheduler continuation before publishing an effect. The existing channel consume operation combines a complete checked copy with infallible consumption; a failed destination copy preserves the queue. A prepared scheduler reply commits without failure after the effect. Return changes only the caller's RAX. Yield and retire require no staged effect. Rejected completion retains the pending request and stage for an exact retry.

A veil fault discards the stage, revokes the pending authority, faults the veil and its blocked dependent caller, and dispatches the unchanged native service. No Linux error is invented by the kernel. This proof uses containment and full teardown; it does not claim restart of a failed veil or continuation of its dependent process.

Three-task owners use explicitly selected const-generic groups. Existing two-task aliases retain their two-slot layout and behavior. The proof owns 75 frames: 48 data frames including transition stacks, 27 table frames and 15 active/bound leases. All tasks and owners must be retired and reclaimed before framebuffer presentation or storage reuse.

## Scope and trust

This relocates runtime syscall/device policy. The existing fixed Linux argc/argv/auxv startup fixture remains in the proof controller and is identified as such in evidence. The previous userspace ELF loader remains separately qualified; this profile consumes the host-qualified artifact and manifest rather than rerunning that loader.

The veil is trusted to implement Linux semantics correctly for its dependent caller. The kernel does not decide whether an errno or poll response is semantically true. Kernel-enforced properties are native ownership, bounded access, exact correlation, failure atomicity and containment. This is one bounded caller/veil relation and two configured record routes, not a global compatibility service, Linux VFS, arbitrary syscall forwarding, libc, Electron or a production process ABI.

## Acceptance

Run the unchanged native reference followed by three veil scenarios: normal editor exchange, producer UD2 with one queued copied surface, and veil UD2 after a staged first surface. Compare all seven surface hashes and final pixels. The normal veil rejects stale stage tokens and invalid completion actions on submit, consume and copyout, then completes the original request. The veil-fault scenario additionally rejects wrong request IDs, duplicate staging and stale completion before crashing. No staged surface may be published on that fault; the service must run with closed routes and all owners must be reclaimed.

Host tests cover sealed scheduler ownership, queue preservation, task-generation mismatch, replay, sequence exhaustion, inactive-root permission/copy failure, protocol encoding and Linux policy boundaries. Target qualification must cover both TCG/Intel and KVM/AMD, 96/320 MiB guests, whole-frame capture comparison, emitted stack review, normal-image exclusion, the preceding ABI proofs and the required regression matrix. Builds remain serialized under explicit memory caps and host OOM monitoring.

## Qualification receipt

All 39 frozen-source matrix jobs and two restoration jobs passed, including TCG/Intel and KVM/AMD 96/320 MiB boots, independent captures, prior ABI proofs, required regressions and normal-image exclusion. All ten preceding artifact blobs remain unchanged. The final kernel and veil match the independently reviewed binaries exactly. Host and cgroup OOM counters remained zero. A bootstrap headroom regression was caught and corrected by constructing the existing pair directly in retained BSS, reducing the older Linux bootstrap chain by 24,112 bytes without enlarging a stack or guard. See the [Userspace Veil Log](../../notebook/Development/2026-10-04%20Userspace%20Veil%20Log.md) for hashes, limits and receipts.
