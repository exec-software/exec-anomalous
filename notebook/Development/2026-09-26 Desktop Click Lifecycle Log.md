# Desktop click lifecycle checkpoint

Date: 2026-09-26

## Scope

The user requested the next logical desktop slice before returning to
foundational kernel work. The chosen boundary is reliable pointer button
interaction: release-to-activate, cancellable presses, and canonical feedback.
See [proposed ADR 0036](../../docs/adr/0036-activate-desktop-controls-on-pointer-release.md).

The workspace now owns public typed control identities and optional armed
feedback. An arm grants no authority and survives only an explicitly rearmed
successor revision. The adapter retains the original control while held and
requires release over that same topmost enabled control. Moving away cancels
on release; returning before release is allowed. Extra-button chords, keyboard
input, source loss, stale revisions, and rejected actions cancel. Immediate
window focus, dragging, and resizing remain available.

All settings and windows remain session-local public built-ins. This change
adds no kernel mechanism, input ABI, independent application process, private
text, storage format, or authenticated external-agent connection.

## Implementation and review

`desktop_target` is the shared typed control identity; the compositor keeps
`chrome_target` as an alias. `arm_pointer` and `cancel_pointer` require observe
and arrange grants. Arming Close or an appearance control does not supply the
manage/customize rights needed by its eventual action. Machine and accessibility
views observe the same optional target as the renderer.

Each prepared candidate starts unarmed, and any non-pointer command in a batch
clears an earlier arm. The adapter explicitly rearms a held continuation after
checking its expected revision, original target, and current availability.
Release additionally requires the same topmost hit. The existing four-command
limit still accommodates two physical facts plus an arm or up to two action
commands. Rejected candidates preserve the committed revision and transaction
sequence; a separate successful cancellation publishes the recovery state.

The renderer requires the current hover, matching canonical arm, and exactly
the left-button mask for pressed paint. Pressing elsewhere and moving over a
button cannot depress or activate it. Leaving an armed control removes its
pressed paint; returning before release restores it. Window focus and geometry
drags remain immediate and use their existing capture checks.

Review identified that a fresh adapter must not assume a released physical
mouse. It now requires an all-buttons-up observation before accepting either
a control press or a geometry drag. Chords and pointer loss impose the same
release barrier. Accepted keyboard makes and keyboard loss cancel local capture
only after source validation; unmapped keys and bounded policy rejection still
publish canonical cancellation when needed. Repeats and key breaks do not
manufacture additional actions or frames.

## Final validation

**Observed:** the final `make check` passed 37 workspace model tests and
8 compile-fail documentation tests, 38 compositor tests, and 34 native input
adapter tests. Warning-denied checks and the repository's configured feature
combinations passed. The compiled frame references remained deterministic.

Model evidence covers observation/grant parity, explicit revision-bound
continuation, invalid or stale identities, hidden/tiled targets, atomic failed
release, and same-batch cancellation. The compositor checks true and false
presses across all 29 actionable controls in Light, Dark, and High Contrast.
Adapter tests cover release-only activation, leave/reenter, no target transfer,
all non-left chords, source loss and first-source neutrality, replacement window
generations, hidden/disabled choices, and canonical keyboard cancellation.

**Observed:** `make workspace-smoke` passed in 180.9 seconds.
It verified 21 exact keyboard frames and 410 pointer/Appearance frames:
124 retained window/pointer frames, 213 Appearance frames, and 73 new
release-gesture frames. The new sequence checks Close survival on press and
drag-away release, exactly-once reentry release, held entry without arming,
settings on matching release, mapped and unmapped keyboard cancellation,
button-chord cancellation, and rejection of a stale Close after slot reuse.
Two desktop sessions, all five containment paths, complete resource restoration,
and working shell fallback remain covered.

The native oracle chooses expected commands independently of the input adapter.
Its frame comparison shares the workspace/compositor implementation and is
cross-build presentation evidence, not an independent full-rendering oracle;
direct semantic and pixel tests provide separate component evidence.

All 20 validation targets passed. The **1020.7-second total** sums the final
passing target durations below; it is not a measurement of total wall time.

| Target | Result | Seconds |
| --- | --- | ---: |
| `make check` | Passed | 246.7 |
| `make smoke` | Passed | 0.4 |
| `make fault-smoke` | Passed | 0.4 |
| `make circle-smoke` | Passed | 31.0 |
| `make control-smoke` | Passed | 30.6 |
| `make rite-smoke` | Passed | 30.7 |
| `make scheduler-smoke` | Passed | 32.8 |
| `make supervision-smoke` | Passed | 30.7 |
| `make shell-vessel-smoke` | Passed | 30.8 |
| `make memory-smoke` | Passed | 39.4 |
| `make interrupt-smoke` | Passed | 31.9 |
| `make input-wait-smoke` | Passed | 32.6 |
| `make shell-smoke` | Passed | 0.6 |
| `make display-vessel-smoke` | Passed | 31.7 |
| `make supervised-renderer-smoke` | Passed | 31.8 |
| `make semantic-surface-transport-smoke` | Passed | 75.2 |
| `make semantic-presentation-smoke` | Passed | 76.0 |
| `make desktop-smoke` | Passed | 54.3 |
| `make workspace-smoke` | Passed | 180.9 |
| `make normal-exclusion` | Passed | 32.2 |

## Final native artifact and interactive session

The inspected final workspace image SHA-256 is
`0875e2747350dcbe30bbe6bb993f7957e24b859432c2c1a7b91efc9b3247be9c`.
A private copy was launched as **Anomalous Workspace - Interaction Checkpoint**,
preserving existing sessions. QEMU PID 1738407 uses
`build/desktop-runs/run-click-1738389/workspace-dev.img`. Its image hash matches the final inspected artifact.
Native service readiness and a complete 768-row first frame were verified;
the private stderr log was empty. This increment changes no kernel source or
native input ABI. VirtualBox was not rerun; its earlier host KVM/AMD-V conflict
remains historical evidence rather than a new result.

## Return to foundations

With basic window management, customization, and cancellable pointer controls
available for hands-on use, further desktop expansion can pause. The next
candidate is a separate bounded two-vessel dispatcher development image with
sustained timer preemption and task-local fault containment. This is a proposal
for the next foundation slice, not implemented behavior or an accepted runtime
architecture.

The current workspace starts its task outside scheduler admission. Its timer
path returns to the same owner until watchdog withdrawal; `task_yield` waits
with `sti`/`hlt`, and an unscheduled task fault withdraws the workspace owner.
The accepted scheduler proof supplies fixed context-switching evidence but does
not establish a persistent multi-vessel dispatcher. A bounded follow-up should
prove repeated progress in two isolated tasks, contain one task's exact fault
while its peer continues, and reclaim both tasks' state and frames precisely.
It should reuse accepted memory, context, task, and supervision invariants while
keeping general admission, wait/wake, service loading, and restart policy as
explicit later boundaries.

The read-only audit used the current
[workspace runtime](../../kernel/src/arch/x86_64/workspace_runtime.rs),
[fixed scheduler context proof](../../kernel/src/arch/x86_64/scheduler_test.rs),
and [accepted ADR 0015](../../docs/adr/0015-prove-bounded-preemption-and-user-fault-containment.md).
No kernel source was changed for this desktop checkpoint.

Private console input must still use move-only ownership and disclosure-aware
views; it cannot be added to this public `Copy` workspace snapshot.
