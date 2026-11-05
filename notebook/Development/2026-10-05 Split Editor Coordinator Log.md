---
title: "Split Editor Coordinator Log"
date: 2026-10-05
tags: [userspace, editor, workspace, ownership, storage]
status: qualified-portable
---

# Split editor coordinator — 2026-10-05

The user authorized the next coordinator slice after eager workspace CPU
ownership. Starting source was clean `70b9918` on `main`, matching origin.
The implementation contract was committed and pushed separately as `15f121e`.

Evidence root:
an external validation directory (not distributed).

## Scope and native gate

ADR 0069 composes the actual portable workspace, sole interactive editor,
compositor and durable store with delayed action resolution and borrowed frame
storage. The feature remains opt-in. It does not select a new guest profile,
port Electron, implement a filesystem, or change native/ELF baseline order.

The native audit found that `owned_execution::prepare_workspace` is a one-owner
profile, while native arena admission and peer switching assume the existing
pair. The workspace mailbox and application scalar calls also enter through
different gates. Mixed owner admission, one selected CPU/IRQ/fault coordinator,
task-bound copied routes and artifact-specific stack qualification remain the
next native gate. Existing single-workspace and pair evidence cannot qualify
that combination by inference.

## Memory policy

Copied the final PSI-aware runner from the prior eager-workspace evidence,
changed its transient unit prefix to `anomalous-editor-coordinator-`, and passed
its pure self-check. No previous evidence is overwritten. Builds and VMs run
serially, with `CARGO_BUILD_JOBS=1`, one Rust test thread, MemoryHigh=4 GiB,
MemoryMax=6 GiB and MemorySwapMax=512 MiB. It monitors system, current and ancestor
cgroup pressure, available memory and kernel OOM counters. Each scope requires
30 seconds of quiet before work. The final audit must additionally inspect
systemd-oomd and transient-unit journals; zero kernel OOM counters alone do not
establish the absence of a userspace pressure-policy kill.

Baseline boot-image hashes were saved before editing in `baseline-images.json`.
The preceding turn's OOM interruption remains documented in its own log; this
turn's outcomes will be recorded separately below.

## Implementation and focused validation

Source checkpoint `cb6eebd` was committed and pushed after the focused checks.
It adds 27 host tests and three compile-fail ownership examples. The
coordinator implements private role capabilities, explicit route direction,
scoped retirement and ready-role rotation. Split submission, resolution and
completion use exact move-only owners; ordinary desktop mutation cannot pass
an unresolved action. The legacy synchronous local API remains available, so
the future native adapter must expose only the coordinated path rather than
mistake this module for OS-wide role enforcement.

The frame storage trait is sealed. The existing owned-frame API remains valid;
borrowed constructors render directly into the supplied buffer, and
`publish_recycling` returns the retired buffer after infallible sidecar install.
Rejected updates and cancellation return the exact retained allocation. Surface
identity exhaustion can return rendered bytes, but not publication or lost
ownership. Malformed observations fail before altering their buffer.

Focused results (`compile-1`, then `focused`):

- Workspace: 96 unit and 14 documentation tests passed.
- Window surface: 19 unit and five documentation tests passed.
- Compositor: 62 unit tests, three earlier bridge integrations and seven new
  coordinator integrations passed.
- Freestanding workspace/compositor checks and warnings-denied Clippy passed.

The seven new composed cases authorize document request/reply routes before
calling the real store or acknowledging the sole editor. They use two stable
heap buffer addresses, literal semantic sequences/versions and full row
composition. They cover saved B with newer C, immutable staged C during the
reply, save C and durable recovery, stale input after a save ACK, continued C!
rendering after service retirement, editor retirement/withdrawal, and accepted
Save draining after workspace retirement. Pixel comparison reuses the renderer
for content and checks independent viewport coordinates; it is not a new glyph
oracle, physical scanout or native task-isolation proof.

Independent source reviews found no blocking fence or recycling issue. Root
froze 428 non-Markdown source/configuration files in `source-freeze.json` before
the full required regression run. The final outcome is recorded below.

## Full regression outcome

All sixteen required targets passed, with `make check` completing in 407.71
seconds. The three serialized smoke groups took 103.06, 168.10 and 232.62 seconds
of job time, respectively; their separate quiet admission intervals are not
included in those durations.

`make check` covers formatting, host and freestanding checks, warnings-denied
linting, artifact inspection, feature-selection/link checks and the VirtualBox
tool's host tests. The other fifteen targets are `smoke`, `fault-smoke`,
`circle-smoke`, `control-smoke`, `rite-smoke`, `scheduler-smoke`,
`supervision-smoke`, `shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`,
`input-wait-smoke`, `shell-smoke`, `semantic-surface-transport-smoke`,
`semantic-presentation-smoke` and `normal-exclusion`. Unit testing the VBox tool
does not claim a new VirtualBox guest boot. No boot implementation changed.

`workspace-smoke` passed in 84.30 seconds: 21 keyboard frames, 465 pointer/
Appearance frames and 26 graphics frames, totaling 512 exact complete-frame
comparisons. The transcript also confirms two desktop sessions, controlled
faults, watchdog and malformed contexts/mailboxes, orderly exit, restored
resources and working shell fallback. This is the existing native desktop
regression; the new coordinator is not enabled in that guest.

Rebuilt the retained-workspace and eager-CPU profiles, then restored normal and
workspace development images. All four 16 MiB images match the pre-change
baseline exactly:

| Image | SHA-256 |
| --- | --- |
| `build/anomalous.img` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` |
| `build/workspace-dev.img` | `f96c18f51f45205b99135d5886e6375ec95006f07d2fb5018a0723e2bf92e046` |
| `build/owned-workspace-test.img` | `07d431becbe325a36403a8491566eed14f782261054b3b8e547952e27bcd1025` |
| `build/owned-workspace-cpu-test.img` | `12d2885c2e634492e174b2a822680630035b6be1a2b0c6fbcb7e9e3195b5cde1` |

`final-audit/receipt.json` passed all seven scopes and 27 command receipts,
independent memory/PSI reconstruction, unit and systemd-oomd journal checks,
428 frozen source files and those four image comparisons. Its SHA-256 is
`1f2d173e6afea03f326962008562b8bd78170077814ffe5964a92daa1c0059a3`.

## Memory and pressure outcome

Maximum scope peak was 2,314,280,960 bytes (2.155 GiB); minimum available host
memory was 19,008,868,352 bytes (17.703 GiB). Scope high/max events and sampled
kernel/scope OOM deltas were zero. All jobs completed without a guard abort, and
the retained accessible systemd-oomd journal contains no victim in this turn's
audit window. Successful transient units were garbage-collected: unavailable
`ManagedOOMKills` values are recorded as unknown, not measured zero. Completion
is inferred from the bound Started record, complete successful runner receipts,
final memory sample and absence of an adverse journal record. Journal rotation
or loss is not disproved.

This run did observe shared-system pressure. In `required-2`, two samples at
23:07:56 and 23:07:57 UTC reported two-second system full-pressure rates of
12.720% and 10.850%. The quiet admission interval reset; later work waited for
recovery. Peak full `avg10` was 3.17%. The scope's own `avg10` stayed zero;
its maximum logged two-second full rate was 0.142%. The pinned runner checks
the abort threshold only while a job is active, and requires 30 continuously
quiet seconds before each job. All five jobs passed; their 168.10 seconds of
execution span 271.24 seconds of logged samples, including admission waits.

The between-job classification follows that pinned control flow and job-log
modification times; these receipts do not independently timestamp the active-job
flag. `pressure-admission-review.json` preserves the observations and that limit,
with SHA-256
`4dcbe4a9406a304563f76f0d5b0cb24c501019d8e1b0fdf15d5819c781430667`.
Do not describe this run as zero pressure. No unrelated service or host OOM
policy was changed.

## Next native work

Provision mixed workspace/editor/service owners and explicit frame storage,
unify their gates/IRQ/fault/CPU ownership, and authenticate copied action/results
and observations through task-bound routes. The local Rust owners are not
serializable capabilities; native adapters must replace direct owner borrows
with admitted transport authority. Qualify exact native code, full stack paths,
physical input/composed pixels, persistence, per-role retirement and cleanup
before exposing a guest launcher entry. The Electron frontend remains unported.
