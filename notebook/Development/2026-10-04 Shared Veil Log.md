# Shared Veil Log — 2026-10-04

## Status

The qualified bounded ABI slice implements two independently owned Linux editor
producers and two native editor services sharing one supervised userspace veil.
Qualification is complete: all 41 qualification checks and two image restorations passed. [ADR 0054](../../docs/adr/0054-prove-a-shared-multi-caller-userspace-veil.md)
records the exact scope. The previous three-task proof and its evidence remain
independently available.

This still exercises the editor-state fixture, not full Antimatter/Electron.
Ny remains the official language direction. HAG, Displacer, native Reliquary
framing, and general process/runtime services are unchanged.

## Implemented ownership and lifecycle

A move-only service handle binds the original scheduler, a nonwrapping service
nonce, the exact veil generation and two exact caller generations. Registration
precedes dispatch. The existing one-caller handoff remains caller-pinned;
shared handoff additionally requires membership. One raw request is pending at
a time, and all memory copies and native channel mechanisms resolve through
that request's sealed caller authority.

Each caller has independent channels, root, transition stack and eager x87/SSE2
state. SCE is enabled only for the selected Linux producer. All existing Linux,
native editor and userspace veil artifact bytes are retained. The wire exposes
only the existing raw request and opaque correlation sequence, with no caller
identity or memory root selector.

Normal caller termination closes only its channels and removes only its
membership. The last member also retires the waiting veil and revokes the
service. A veil fault aborts staging, revokes the service, faults the current
blocked caller and any queued dependent caller, and preserves both unrelated
native services in FIFO order. No stage is published by fault cleanup and no
Linux errno is synthesized in the kernel.

The five retained owners use 125 frames: 80 data, 45 table, and 25 leases.
The complete runtime remains in retained BSS until bootstrap restoration and
explicit teardown. Concrete two-task owner aliases remain unchanged.

## Isolation observations

Every actual native veil mechanism checks the other pair before and after the
operation: both saved frames, full 8 KiB writable data and 8 KiB user stacks,
channel payload bytes and metadata, acknowledgement state and effect counters.
These are private proof observations, not values exposed to the veil.

On each cross-caller handoff the controller submits an old request sequence to
the same `veil_call` query and completion admission used by real userspace gates.
Both are rejected with no effect. These are explicitly controller-injected
negative probes; the unchanged veil binary supplies its existing startup,
copy-failure and staged-completion probes.

Every healthy caller's seven surface-state hashes and 24 final output rows are
compared to the native baseline. Retained baseline row hashes allow both pairs
to share the existing presentation staging buffer. The final capture is checked
against the entire baseline framebuffer.

## Exact compiled-code review

The reviewed kernel is
`bbf117583231965f3b6c082c84c11af4dad0075b615819d3243ddb0b65ce86b5`.
The unchanged veil is
`f5921e5e48c0e50dbcab9ccfc6ed836402314785c5fa021ede010c444e31a554`.

The construction split reduces the largest inspected bootstrap chain from
51,624 to 41,088 bytes of 65,536, leaving 24,448 bytes. The mapping-preparation
branch uses 33,976 bytes; the native-pair comparison in this image uses 28,776.
The largest inspected supervisor transition uses 11,799 of 16,384 bytes, leaving
4,585. The raw SYSCALL path uses 9,735 bytes. These are conservative emitted
admitted-path estimates, with hash-bound manual indirect-call resolution and
local jump-table inspection; they are not formal arbitrary-fatal-path maxima.

The review checks the trusted-stack switch before the first push, producer-only
SCE selection, exact caller continuation and complete CPU/MSR restoration. All
five FP images are wiped. The loaded kernel occupies 294,912 bytes and its
resident end is `0x1ae000`, within the unchanged 384 KiB staging and 2 MiB proof
residency limits. The unchanged veil retains its preceding 1,064-byte user-stack
bound; Linux/native vessel bounds are likewise unchanged.

The preceding Linux and one-caller veil kernels were independently rechecked.
Their bootstrap bounds remain 25,792 and 36,168 bytes respectively, and largest
supervisor paths remain 7,079 and 10,807 bytes. The older veil's raw SYSCALL
path grows to 3,479 bytes including entry, still within its existing 16 KiB
stack. These receipts bind the actual retained regression binaries; they do
not infer safety solely from unchanged source aliases.

## Source-derived scenario oracle

| Scenario | Raw/completed | Stages | Native rejections | Cross-caller handoffs | Stale rejections | Isolation checks |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Both callers normal | 130/130 | 80 | 17 | 15 | 30 | 559 |
| First producer faults | 71/71 | 41 | 14 | 1 | 2 | 300 |
| Veil faults after staging | 6/5 | 1 | 12 | 0 | 0 | 26 |

A healthy caller makes 65 raw calls and stages 40 effects (7 submits, 6 consumes,
27 poll copyouts). The single shared veil performs three startup probes and six
successful-effect retry probes only once. Each healthy caller contributes four
copy failures. Thus normal rejections split 13/4; producer-fault rejections split
6/8. FIFO application yields produce 28, 14 and 0 cooperative switches.

Normal completion exits all five tasks. The producer-fault scenario has four
exits and one fault; the second pair completes all seven states. The veil-fault
scenario has two native-service exits and three faults, with one aborted stage
and zero published/discarded packets. The second producer is admitted but never
runs in this scenario. `application_tasks=15` counts the retained task lifetimes
across the three scenarios, not fifteen executed entry loops.

## Findings during implementation

The new host fault test caught a ready-but-never-run dependent whose scheduler
context remained marked initial after faulting. Terminal cleanup requires a
saved context marker. Shared-fault commit now normalizes the exact dependent
contexts before revocation; tests cover both initial and previously-run peers.
All 487 focused kernel tests subsequently passed.

The first linked five-task image exceeded Threshold's unchanged 384 KiB staging
window by 8 KiB. Diagnostic linking retained a map but produced no bootable
oversized image. Only the new profile now uses linker section garbage collection
to remove unreachable compiler-library functions. Linked artifact inspection
still requires all twelve embedded blobs, and preceding profiles retain their
original linker behavior.

Independent emitted inspection predicted a bootstrap stack-headroom rejection,
confirmed by the first TCG run's `owned_bootstrap_state` diagnostic. Constructor
temporaries in `initialize` overlapped mapping preparation. An outlined
`initialize_storage` now returns before preparation; no stack size or guard was
weakened. Failed-run logs, map and first candidate are retained separately.

The first full matrix passed five ABI smoke jobs, then strict Clippy rejected
a nested conditional in a new verifier mutation test. The test now uses a
combined `if let` condition. Runtime code and reviewed binaries were unchanged.
The failed matrix receipts were preserved, source was frozen again and a fresh
full matrix passed 40 jobs before a shared-KVM timeout described below;
no failure was omitted from the retained evidence.

The first full matrix also reached the 4 GiB soft memory limit while retaining
compiler file cache (3.23 GB file cache versus 75 MB anonymous memory in the
recorded post-reclamation sample). Service swap was zero in that sample. Its
peak was 4,296,994,816 bytes, with 2,189 soft-limit events and zero hard-limit
or OOM events. That capped failed-check attempt remains part of the resource
record; the successful requalification has its own separate receipt below.

The fresh matrix passed its first 40 jobs, but the final shared-veil KVM job
exceeded the unchanged 30-second deadline. Bounded monitor snapshots showed
changing region/hash registers at the per-word protection-check RDMSR, proving
progress through the observation loop. Full peer snapshots performed about
7.25 million EFER reads across the three scenarios; these were costly on KVM.
No OOM or memory-limit event accompanied the timeout.

The repair preflights each complete retained 8 KiB region, streams every volatile
word in the same little-endian FNV order, and rechecks current-root protection
state afterward. It exposes no pointer or callback and runs only during the
active retained interval with interrupts excluded. Review caught and corrected
an initially inverted active-selection guard before the first rebuilt run.
A new host test covers boundary/interior changes, neighbor independence and
byte-hash equivalence. Neither the timeout nor observation coverage was relaxed.

Only three files changed after that matrix: shared runtime and two strictly
shared-profile observation helpers. The exact earlier source and failure remain
under `qualification-attempt-2/`. A scope review supports retaining 38 unaffected
checks; the three affected checks (`make check`, shared TCG and shared KVM) were
rerun on the final frozen source. `qualification-results.json` records the source
snapshot and receipt used for each of the 41 qualified checks, without replacing
the original failed matrix receipt.

## Validation and resources

All **41 qualification checks** passed, followed by both image restoration
jobs. The original matrix passed 40 jobs and timed out in shared KVM; after the
three-file shared-profile observation repair, `make check` and shared TCG/KVM
were rerun on **327 unchanged frozen source/configuration files**. The 38
unaffected checks retain their preceding receipts and source hashes, with the
scope review and per-check provenance recorded in the evidence. This covers all
repository-required checks, prior ABI and desktop regressions, TCG/Intel and
actual KVM/AMD 96/320 MiB boots, and independent framebuffer captures.
Host tests pass 488 kernel tests in the shared profile, 522 with all features,
and 160 xtask tests. These include the new full-region hash test. All twelve preceding embedded artifact blobs are unchanged.
Whole TCG/KVM captures match and the final kernel equals the independently
reviewed binary. Normal and workspace development images are restored.

The final affected-check rerun peaked at 4,296,867,840 bytes
(4.002 GiB), with minimum host available
memory 14,322,757,632 bytes
(13.339 GiB). Its memory-limit events
were 2700 soft and 0 hard.
A retained soft-limit sample contained 3,294,806,016 bytes of file cache,
790,302,720 bytes of anonymous memory and 4,653,056 bytes of service swap.
Soft-limit reclamation completed without a hard-limit or OOM event.
No host or cgroup OOM occurred across any attempt. Actual KVM invocations total
32, including the retained shared-veil timeout and three
successful final shared-veil boots/capture.

| Artifact | SHA-256 |
| --- | --- |
| anomalous.img | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| workspace-dev.img | `43af7a925902c15db590209dd53769d6a20c2eec22aa48d1a0ae76dce7df1620` |
| shared-veil-test.img | `99d744b903abd6657318f59aec8c94b2ec17bf7cd4adc4afc7e75f41114ce6ba` |
| Reviewed shared-veil kernel | `bbf117583231965f3b6c082c84c11af4dad0075b615819d3243ddb0b65ce86b5` |
| Unchanged veil vessel | `f5921e5e48c0e50dbcab9ccfc6ed836402314785c5fa021ede010c444e31a554` |

Evidence is retained under
an external validation directory (not distributed).

All builds and QEMU jobs are serialized with `CARGO_BUILD_JOBS=2`, Make `-j1`,
`MemoryHigh=4G`, `MemoryMax=6G`, and `MemorySwapMax=512M`. The runner samples every
two seconds and terminates its process group on a new OOM event or less than
4 GiB host available memory. All retained attempts report zero host and cgroup OOM events.

## Next gate

Expand native application runtime facilities against Antimatter's requirements:
startup, memory/thread/TLS ownership, event delivery, filesystem access and richer
window/content presentation. The shared veil remains a fixed two-member serial
proof; asynchronous multi-request services and restart are separate gates.
