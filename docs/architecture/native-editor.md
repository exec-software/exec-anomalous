# Bounded native editor models

The user clarified the application target in
[ADR 0073](../adr/0073-run-linux-antimatter-through-the-linux-veil.md): run the
existing Linux Antimatter build through the Linux compatibility backend.
The native reference components below supply regression evidence; they are not
a frontend port or a requirement to convert Antimatter to `.re`.

The editor models are userspace prerequisites for the native Antimatter direction
in [ADR 0045](../adr/0045-select-native-antimatter-ny-and-hag-direction.md).
They are small Rust reference components, not the complete Electron/React/
CodeMirror frontend. The workspace Editor Lab exercises physical keyboard focus
and live rendering for public memory-only scratch text under
[ADR 0064](../adr/0064-add-a-public-keyboard-editor-workspace.md). The separate
interactive durable pair in
[ADR 0065](../adr/0065-prove-an-interactive-native-durable-editor.md) has passed
bounded native qualification, including physical editing and cold recovery.
The Linux Antimatter copy
remains the behavior reference; its independently changing working tree is not
modified by these gates.

## Preserve the qualified model

`editor_state`, `loaded_editor_state` and `opened_editor_state` keep their
existing one-entry undo behavior: a second changed insert fails until undo
consumes the entry. Their layout, constructors, error meanings and earlier
native/ELF proofs remain the reference for that profile. Saving never silently
clears undo or changes the current editor revision, selection or appearance.

The opt-in `repeated_opened_editor_state` owns an opened state and permits
consecutive changed edits. It deliberately retains **one immediate predecessor**,
including the predecessor's selection. A successful changed edit replaces an
older undo entry only after validating text, resulting capacity and revision.
`edit_effect::changed { discarded_undo }` explicitly reports that replacement.
This is a local retention policy, not a claim of unbounded undo, redo or
CodeMirror history parity. A caller must choose this model explicitly.

The implementation stages a copy of only the local editing state, then invokes
the existing insertion validation on that candidate. It publishes the candidate
only after success. Rejected edits preserve text, selection, revision, appearance,
undo, saved bytes and any pending save. A content no-op preserves the previous
undo entry and revision; as in the original model, it may collapse selection to
the end of the inserted range. `edit_effect::unchanged` refers to text content,
not necessarily caret position. Undo restores the immediate predecessor and
consumes the entry while advancing the revision; appearance remains unchanged.

The wrapper is neither `Copy` nor `Clone`. Its `state()` view is read-only. No
mutable underlying-state escape allows callers to replace the saved baseline
or fork a pending request. This is a safe Rust API, not a serialized layout.

## Saves retain exact identity

The wrapper delegates all save operations to the existing opened model from
[ADR 0056](../adr/0056-prove-task-bound-native-document-open-and-save.md).
Beginning a save copies the precise text/revision and reserves its request ID.
Further local edits cannot replace that pending snapshot. Only an exact
correlated acknowledgement advances the saved baseline and store version.
Acknowledging B after an edit to C saves B and leaves C dirty. Saving C next
can make C clean; undoing afterward is dirty relative to the saved C baseline.

An unknown outcome remains unknown while local editing continues. The exact
pending snapshot stays retained, a second save stays blocked and returning to
previously saved text cannot make an uncertain editor clean. A negative response
cannot resolve a request already marked unknown. Reopen/reconciliation requires
fresh authenticated service state; it is not inferred from text equality.

## Native repeated-save proof

`native_repeated_editor_test` selects an alternate client built with
`repeated_proof` and reuses the existing native durable runtime, copied routes,
startup descriptor, task ownership, checked copies and sector gates. There is
no new privileged operation, document wire version or disk format. The service
continues to own all format/mount/checkpoint policy over the temporary mechanism
specified in [ADR 0063](../adr/0063-prove-native-durable-document-storage.md).
The shared runtime emits its existing `kernel.native_durable_test` evidence;
the separately selected image and strict repeated-session host oracle establish
which application trace is required. Higher failure/relaunch/ELF profiles keep
explicit selection priority.

A fresh disposable extent starts with empty A at version one. A host challenge
B reaches only the initial editor and is constructed by three consecutive
inserts. While B's save is pending, replacing its first byte produces C directly.
The service acknowledges B, reads back B while C remains dirty, saves C, then
observes undo to B and a last-byte edit to D. Saving D advances content version
to four, crossing both checkpoint slots multiple times. Both userspace owners
retire before remount, then a new VM recovers D without receiving its expected
bytes. The host predicts packet contents, revisions, saved versions, media and
rendered pixels independently. Such a trace remains scripted; it does not
establish keyboard routing or interactive Antimatter execution.

The existing wire profile still admits one printable ASCII line, 31 text bytes,
and undo depth zero or one. These are fixture-local capacities. They introduce
no filesystem name, path or depth limit and do not become general editor limits.
Qualification evidence belongs in the
[Repeated Native Editing Log](../../notebook/Development/2026-10-04%20Repeated%20Native%20Editing%20Log.md).

## Live integration gate

Editor Lab consumes copied physical input and presents acknowledged complete
frames through the existing workspace. Focused editor routing prevents text keys
from triggering desktop shortcuts, handles input loss, and blocks physical
editing when the chosen text scale cannot fit. Legitimate edit rejection produces
canonical feedback while preserving local document state. The emitted workspace
has a qualified 61,240-byte conservative entry bound within its 64 KiB stack;
future changes require artifact-specific review.

The [Keyboard Editor Workspace Log](../../notebook/Development/2026-10-05%20Keyboard%20Editor%20Workspace%20Log.md)
records the 95-frame QEMU trace, existing workspace regressions, VirtualBox
initial-frame comparison and memory guard. The co-located public component has
no save authority and is not independently isolated Antimatter. Its existing
qualified behavior remains distinct from the new durable pair.

`local_editor_state` is the copyable scratch-only model used by that built-in.
It shares staged repeated insertion and adds atomic Backspace/Delete helpers,
preserving the original selection in undo. It carries no document identity or
pending save. Its compact desktop projection exposes text, caret, selection,
revision, dirty state, history depth and structured feedback. Ctrl+S explicitly
reports Save unavailable. Closing discards the scratch contents. The existing
opened models and durable service remain the reference for the next integration.

## Interactive durable session — bounded native qualification

The opt-in `native_interactive_editor_test` profile uses a dedicated editor task
and a separate durable service, with the client built using `interactive_proof`.
Only the editor receives copied physical input and direct display staging; only
the service can read, write or flush the provisioned disk extent. Four existing
opaque routes retain exact task bindings. Userspace owns document decoding,
save policy and rendering. This is a separate fixed-region proof, not a change
to the workspace's scratch Save behavior.

`interactive_editor_session` retains one non-copyable repeated opened model.
The opt-in opened wrapper now also offers caret movement and staged Backspace/
Delete without changing saved or pending state. A Copy `interactive_editor_view`
contains observations only. It exposes local revision separately from its
session sequence, acknowledged store version, pending request/revision, dirty
state, save outcome, history depth, feedback and close confirmation. Selection,
input reset and save completion may advance the session without a text edit.

Local edits and save-result transitions are serialized in that owner. A display
failure cannot roll back an already acknowledged durable save. The client
renders one exact immutable view and identifies the corresponding presentation;
the copyable desktop snapshot never contains the save owner. A dirty, pending
or uncertain session needs a second consecutive close; intervening processed
actions clear confirmation. Local close does not pretend to revoke routes or
cancel a disk commit.

The exact `ANDI` 192-byte startup and `ANDF` 64-byte provisioning record contain
phase and proof mode, no text or recovery answer. The service obtains fresh
mount identity and content from the durable model. `ANUI` is a separate 128-byte
public-fixture audit, carried on route 0 and checked against literal host
expectations. It is not authority or a policy for disclosing private files.

The editor renders 128 by 48 RGB8 pixels, scales by three, stages all 144 rows
through gate 7 and commits the complete `(32, 32, 384, 144)` region through gate
16. Staging is private and incomplete rows cannot publish. Direct rendering
remains available after the service exits and packet routes close, so an
unknown save and newer dirty text remain visible.

Presentation first validates the exact retained task, admitted root, selected
TSS stack and live high-alias stack extent. With interrupts disabled and the
user CPU image already captured, the dedicated helper temporarily selects the
bootstrap CR3 only for the integer framebuffer/MMIO callback. It performs no
user copy, dispatch or ownership change in that scope. The callback preserves
the stack and CPU contract; the helper restores and revalidates the exact task
root before any userspace return. A violated precondition, callback contract or
restore check fails closed while retaining ownership, with no retry from an
uncertain root. Emitted inspection must bind this path to the final binary.

Mode 0 supplies ordinary durable replies. Mode 1 retains a committed snapshot's
reply until a newer text audit arrives, proving that late acknowledgment leaves
the newer buffer dirty. Mode 2 commits and drops the reply before service exit;
the survivor retains the exact pending snapshot as unknown. A separate cold VM
mounts the same data disk without being given the expected bytes. Recovered
content is not a durable request-receipt ledger.

The idle gate may sleep only after an empty editor input read, empty/closed
routes and an IF-clear recheck of the input queue. `STI; HLT; CLI` permits input
or the next PIT tick to wake it, then a live peer receives the cooperative turn.
The timer bounds delay when that peer has private pending work. This is a
profile-specific interval, not general asynchronous wait or timer dispatch.

The bounded native gate passed eight QEMU boots, 62 complete frame checks,
48 stable captures and independent cold-media verification. Exact emitted review
bounds the client at 4,928/8,192 stack bytes and the kernel transition plus idle
IRQ at 9,662/16,384. All 16 required checks and affected native/ELF/workspace
regressions passed. The
[qualification log](../../notebook/Development/2026-10-05%20Interactive%20Durable%20Editor%20Log.md)
records the exact artifacts and limits. This trace does not newly qualify
input-loss recovery or hostile service faults, and after service exit no ANUI
route remains to attest hidden metadata separately from predicted pixels.

A scoped workspace bridge and ELF comparison follow this native baseline. The
real Antimatter frontend/runtime, private-document policy, larger documents and
filesystem namespace remain separate work.

## Portable workspace bridge

[ADR 0066](../adr/0066-model-an-owner-bound-editor-workspace-bridge.md) adds the
opt-in `native_editor_bridge` feature to the workspace and compositor. It gives
the native reference editor its own window kind, distinct from memory-only
Editor Lab. For this window, desktop snapshots retain only geometry and a
surface descriptor. `native_editor_owner` retains the sole session, and
`native_editor_bridge` retains the authorized observation and owned immutable
RGB pixels separately.

Trusted local setup explicitly selects `public_fixture` disclosure and binds the
exact desktop, producer endpoint, window incarnation and nonwrapping attachment
generation. The live bridge checks observer grants before returning text or
pixels. No copied descriptor supplies read, input, save or publication authority.
This is a local Rust ownership contract; it authenticates no native task/channel.

One move-only update contains the next observation, pixels and desktop ticket.
Complete presentation precedes descriptor/sidecar publication. Cancellation
returns the pending frame for retry and never rolls back the actual editor or
durable save. A foreign publication rejection returns the complete update.
Actions use a separate one-shot permit and recheck current focus, modal state,
grants, desktop revision and the real editor's session sequence at consumption.
An intervening save reply requires resynchronizing a stale action. Foreign
action rejection preserves the rightful owner's ticket; locally owned stale
actions are consumed without effects.

Normal close follows the editor's close policy: clean idle state closes
immediately, while dirty, pending or unknown state requires confirmation. Only
its exact closed observation removes the window. A separate controller assertion
can withdraw a terminal application's window without claiming save success.
Fresh attachment/window generations reject old work. A surviving owner cannot
yet reattach to a replacement desktop. The host integration exercises the real
durable model and complete compositor, but no native VM receives this feature
or a new launcher entry.

Native integration first needs explicit workspace execution and storage
admission: its existing 64 KiB user stack does not fit the application's 8 KiB
profile. [ADR 0067](../adr/0067-qualify-retained-native-workspace-admission.md)
implements that separate retained single-workspace profile with the bridge
disabled; its [qualification log](../../notebook/Development/2026-10-05%20Owned%20Workspace%20Admission%20Log.md)
records the evidence. ADR 0070 below qualifies a separate nine-page arena for
the two 18,432-byte content frames outside its remaining stack margin.
[ADR 0068](../adr/0068-qualify-eager-workspace-cpu-ownership.md) adds the separate eager workspace CPU profile so workspace, editor and service
can share an explicit CPU ownership contract. ADR 0070 qualifies one three-owner
runtime and task-bound routes with small probes; ADR 0071 below adds the opt-in copied action/observation adapter. Its native
transport binding and real payload selection still follow. Workspace admission and CPU ownership alone do not
connect the native editor window.

The coordinator must keep workspace display/input, editor session ownership and
service disk access separate. A submitted editor action holds a desktop
publication barrier until its exact result or trusted terminal withdrawal;
the workspace remains schedulable to pump the reply. Transport acknowledgement
cannot release that barrier or imply a durable save. Service retirement must
close only document routes so newer dirty/unknown state can remain visible.
Editor retirement permits explicit window withdrawal without claiming a save
or user-confirmed close.
The [portable qualification log](../../notebook/Development/2026-10-05%20Editor%20Workspace%20Bridge%20Log.md)
records passing host composition, required checks and unchanged default native
artifacts, with separate desktop QEMU/VirtualBox regression evidence.

## Split coordinator and retained frame storage

[ADR 0069](../adr/0069-compose-a-split-editor-workspace-coordinator.md) qualifies
the composed portable gate. Workspace, editor and document service have distinct
local role capabilities and four directed routes. Retiring the service closes
its document routes while preserving editor/workspace communication. Workspace
retirement stops new input without pretending to cancel a previously admitted
save. These local capabilities still need native task/channel bindings.

Submitting an action installs a desktop-wide publication fence. The editor can
resolve the move-only submission independently; the workspace consumes its exact
completion before changing focus, modal context or grants. Committed observation
and reply processing stay available while fenced. A service reply that advances
the editor first makes the old expected sequence stale; its rejection completes
the action without an edit. Packet acknowledgement and timeout cannot clear the
fence. Trusted terminal editor withdrawal remains distinct from a confirmed
close or durable save.

The borrowed frame path retains caller-supplied pixel storage through the same
bridge/update/compositor contract. Complete publication returns the old frame
for reuse, and cancellation returns the pending frame. Neither return moves a
pixel array. The two exclusive buffers require 36,864 bytes outside the native
workspace stack; their mapping, metadata and complete emitted stack use remain
part of the mixed native admission gate. The default owned-frame path remains
available for existing host callers.

The portable coordinator is not yet connected to guest payloads. ADR 0070 below
provides the mixed mapping, entry-gate, CPU, route and cleanup mechanism using
small native probes. The real workspace/editor/service adapters and role-specific
device/display grants remain to be integrated and qualified together. The earlier
single-workspace and editor/service proofs cannot be combined by merely
increasing the task count.

Submitted/completed Rust owners are local capabilities, not packets to serialize.
The native adapter must derive remote action/result authority from installed
task/channel incarnations and correlate complete copied observations, replacing
the local bridge's direct owner borrows. Public role IDs, a copied Rust layout
or an ANUI sequence cannot reconstruct those capabilities.

The [qualification log](../../notebook/Development/2026-10-05%20Split%20Editor%20Coordinator%20Log.md)
records the composed host cases, all sixteen required checks, 512 existing native
desktop frames, unchanged boot images and the guarded memory/journal audit.

## Native mixed ownership gate

[ADR 0070](../adr/0070-qualify-mixed-native-workspace-admission.md) selects a
separate three-task admission proof. The retained workspace profile adds a
nine-page RW/NX frame arena at `0x810000`, surrounded by an absent lower gap and
upper guard. Its two pixel slots occupy that arena exactly; descriptors and
other metadata stay in the ordinary data region. The editor and document roles
retain their smaller private arenas and stacks. Legacy constructors retain their
original geometry.

One selected runtime owns the three roots, transition stacks, scheduler and
eager CPU images. Four directed task-bound channels keep workspace/editor
traffic separate from editor/service traffic. A contained service fault closes
only its incident channels; the other two tasks must continue communicating
before final restoration and release. The workspace checks its full 36,864-byte
frame arena before and after peer execution without refilling it, including the
exact two packet words that legitimately change. Service retirement is exercised
with drained queues; pending-save failure remains a later integration case.

This checkpoint uses small native assembly probes to qualify the combined
mechanism. Its workspace `f3` entry is a proof operation, and its role capability
probe performs no disk access. It does not execute the actual workspace, editor
session, durable store, physical-input adapter or compositor. ADR 0071 below
composes the remote application adapter on the host, and ADR 0072 connects the
real payloads through installed native routes. The
[mixed admission log](../../notebook/Development/2026-10-05%20Mixed%20Native%20Workspace%20Log.md)
records implementation and qualification evidence.

## Copied remote editor adapter

[ADR 0071](../adr/0071-compose-a-copied-remote-editor-adapter.md) implements an
opt-in copied application boundary without borrowing the editor into the
workspace. An experimental 64-byte ANEA request and 192-byte ANER response carry
explicit action/sequence correlation and unchanged ANUI observations. Only the
editor owns its interactive session and pending save snapshot. The workspace
retains a controller-provisioned remote port, copied observations and borrowed
frames. Neither a public connection ID nor decoding ANUI proves sender authority.
The trusted transport embedding must authenticate the installed task/channel
incarnation before invoking checked admission or editor execution.

Exact completion clears the existing desktop action fence. A newer observation
may be received while fenced or while an older frame is pending; an older exact
completion never rolls it back. Further input waits until the newest accepted
view is displayed. Identical observations are idempotent; conflicting bytes and
reopening closed state are rejected. Frame admission checks against the latest
receipt, and pending/displayed buffers remain exclusively retained until normal
publication or cancellation. Terminal editor withdrawal invalidates its route
before releasing the fence and cannot imply a durable save.

The [Remote Editor Adapter Log](../../notebook/Development/2026-10-07%20Remote%20Editor%20Adapter%20Log.md)
tracks the copied host composition with the actual editor, durable store and row
compositor. That checkpoint selected no guest profile. ADR 0072 below replaces
pair-specific route assumptions, assigns workspace-only input/display and
service-only disk rights, and qualifies emitted ownership, stacks, physical
input, composed pixels, late saves and cold recovery together.

## Opt-in native desktop integration

[ADR 0072](../adr/0072-integrate-a-native-editor-workspace.md) connects the copied
remote adapter to the installed native channels. One coordinator retains all
three roots, transition stacks and eager CPU owners. The workspace has the
physical input and presentation grant, the editor has the sole session, and the
document service has the sole temporary storage-device grant. Service loss leaves
the workspace/editor routes live and preserves an uncertain save as unknown.

The experimental ANEW startup descriptor supplies installed role/route
correlation. Its fields do not grant authority; the kernel authenticates every
operation against the selected owner. ANDH is a qualification-only delayed-reply
marker on the editor/service route. Neither changes ANDC or the durable disk
format. This profile is separate from the default desktop and the older native
pair. Its 16-boot TCG/KVM matrix at 96 and 320 MiB checks 140 complete desktop
frames, delayed saves, continued editing after a lost committed reply, cold
recovery and exact retirement of all three owners. The
[Native Editor Workspace Log](../../notebook/Development/2026-10-07%20Native%20Editor%20Workspace%20Log.md)
records artifact, regression and resource evidence. It does not establish a
stable `.re` container or complete Antimatter runtime support.
