# ADR 0069: Compose a split editor/workspace coordinator

- Status: Proposed; portable composition qualified, native integration pending
- Date: 2026-10-05
- Related: ADR 0065, ADR 0066, ADR 0067, ADR 0068

## Context

The retained workspace now has explicit mapping and eager CPU ownership. The
interactive native editor and durable service have a separate qualified pair
runtime. Combining them is not a cardinality change: workspace admission uses
different region geometry and a different gate, while the pair assumes that the
other participant is `1 - selected`. Its terminal policy also closes routes that
a surviving workspace and editor need.

The portable bridge consumes actions while borrowing both desktop and editor.
That synchronous check cannot enforce focus after an action crosses a task
boundary. Its two 18,432-byte frames also cannot fit the workspace's remaining
4,288-byte stack margin or one existing 32 KiB private application arena.

## Decision

Compose the split ownership protocol with the real portable desktop, editor,
durable store and compositor before selecting it in a guest. Keep the existing
`native_editor_bridge` opt-in feature and its default owned-frame API. Add a
caller-supplied storage path whose live bridge and update values retain exclusive
borrows rather than pixel arrays. This checkpoint introduces no serialized ABI,
kernel parser, task admission or native launcher entry.

### Keep roles and routes explicit

Use distinct workspace, editor and document-service roles. The workspace owns
physical input and desktop presentation, the editor owns its sole mutable
session, and the service owns the provisioned document device. Local role
endpoints are privately branded; copied role labels or public fields cannot
manufacture an endpoint. These are trusted local Rust capabilities, not proof
that an OS task sent a packet. A native adapter must bind them to admitted task
and channel incarnations.

Keep four directed routes: workspace to editor actions, editor to workspace
results/observations, editor to service requests, and service to editor replies.
Retirement closes only routes incident to the retired role. Service loss leaves
workspace/editor communication available. Workspace loss stops new editor
actions while allowing an already submitted save to finish. Editor loss permits
trusted window withdrawal without a successful-save or confirmed-close claim.
Retirement is terminal for that coordinator incarnation; replacement requires
fresh provisioning.

A publication fence never parks the workspace. It must remain eligible to run
and drain the exact result that releases the fence. Role selection must also
continue among the surviving participants rather than assuming one fixed peer.

### Resolve an admitted action before changing its desktop context

Submission requires the live granted client, focused visible window, binding,
desktop revision and presented editor sequence, with no pending desktop update.
It installs one publication fence and transfers a move-only action to the
editor side. While fenced, ordinary desktop preparation, presentation and grant
changes return backpressure. Committed observations remain readable and result
processing remains possible.

The editor checks its actual sequence at consumption. An intervening service
reply can make a correctly delivered action stale; that action resolves without
an edit and requires a fresh observation. Both success and exact rejection are
completed outcomes. Only the matching completion releases the desktop fence.
Foreign outcomes retain their rightful ownership; dropping a submitted action,
packet acknowledgement or elapsed time cannot release or cancel it.

Save action completion reports snapshot reservation, not durable success. The
sole editor keeps the pending snapshot and only an exact document-service reply
can acknowledge it. A newer edit remains dirty when an older save completes.
Service loss preserves a pending unknown save and permits further local editing
and presentation. The coordinator must not manufacture a negative save reply.

A separate controller assertion of terminal editor withdrawal invalidates the
old bridge and releases its fence so the ordinary window-removal transaction can
proceed. It cannot undo an effect already performed. This local assertion must
be backed by actual task/route retirement in the later native adapter.

### Retain two frames outside value returns

The borrowed path renders directly into exclusively supplied storage and keeps
the validated observation with an immutable surface descriptor. It allocates
no pixel array in its constructors, action results, update tickets or error
returns. Retain the previous frame until successful complete publication; return
its storage for reuse afterward. Cancelled or rejected updates return their
retained frame without changing the editor or committed observation.

At most one committed and one successor frame are needed. Distinct exclusive
borrows prevent a pending composition from overwriting displayed storage. The
native mapping gate must still provision at least 36,864 bytes of pixel storage
outside the user stack, plus separately accounted metadata and alignment. A
host heap allocation or small `size_of` result is not emitted native stack
qualification, a retained mapping, or a new general application memory limit.

## Qualification

Exercise the composed path with caller-owned frame storage, actual desktop
transactions and row composition, a sole interactive editor and a durable model
whose writes survive only successful flush. Cover delayed action resolution,
focus/modal/grant backpressure, stale expected sequences, foreign results,
per-role retirement, and scheduling while the workspace is fenced.

Verify Save B followed by edit C and late acknowledgement, immutable pending
frames, cancelled publication, service-loss unknown state, and terminal editor
withdrawal. Assert literal semantic expectations and meaningful pixel/geometry
results; shared rendering alone is not an independent glyph oracle. Check
borrowed buffer addresses and compact retained values as API properties, without
claiming an emitted stack bound. Existing bridge tests must remain valid.

Run all sixteen repository-required checks, the relevant desktop regression,
and compare rebuilt default native images with their recorded baseline. Keep
builds and VMs serialized with one build job, a 4 GiB soft / 6 GiB hard cgroup
limit, 512 MiB scope swap, and system/ancestor PSI monitoring. Record actual
scope/journal outcomes, including failed attempts.

## Remaining native gate

[ADR 0070](0070-qualify-mixed-native-workspace-admission.md) now implements one
runtime for three mixed layouts, a workspace frame arena, selected-task
gate/IRQ/fault and eager CPU ownership, task-bound routes and role-specific
retirement. Its small native probes are qualified under TCG/KVM. They do not
execute this portable coordinator, the workspace renderer, an editor session or
a document service; the portable qualification above remains a host result.

The next integration must execute the real workspace, sole editor session and
durable service through that owner. Qualify their final emitted user/kernel
control and stack graphs, physical input, composed pixels, delayed saves,
service-loss behavior, disk recovery and complete cleanup under the selected VM
matrix. Admission of the probes does not establish native window rendering or
an Electron/Antimatter port.

The submitted/completed Rust owners are process-local capabilities, not wire
packets. Native action/result admission and copied observations must derive
authority from installed task/channel incarnations, replacing direct local
owner borrows. Serializing their Rust layout, public role IDs or ANUI sequences
cannot supply that authority.

## Qualification result

The composed host path, freestanding checks, warnings-denied linting and all
sixteen required repository targets passed on source checkpoint `cb6eebd`.
The existing workspace regression checked 512 complete frames. All four rebuilt
normal/workspace boot images match their starting bytes, and 428 frozen
non-Markdown source/configuration files remained unchanged through validation.
The seven guarded scopes completed without an OOM kill or aborted job, with a
2.155 GiB maximum scope peak. A shared-system pressure burst delayed quiet
admission between jobs; it was not hidden as zero pressure. Exact results,
image hashes, memory observations and journal limitations are recorded in the
[qualification log](../../notebook/Development/2026-10-05%20Split%20Editor%20Coordinator%20Log.md).
