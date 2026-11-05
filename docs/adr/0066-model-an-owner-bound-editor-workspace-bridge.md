# ADR 0066: Model an owner-bound editor workspace bridge

- Status: Proposed; portable host implementation qualified, native integration pending
- Date: 2026-10-05
- Related: ADR 0027, ADR 0038, ADR 0056, ADR 0064, ADR 0065

## Context

[ADR 0064](0064-add-a-public-keyboard-editor-workspace.md) supplies a public,
memory-only scratch editor inside the workspace. Its copyable local model has
no document or save authority. [ADR 0065](0065-prove-an-interactive-native-durable-editor.md)
qualifies a separate native editor and durable service. Its non-copyable session
retains exact pending save bytes while newer edits continue, but its direct
display region is not a desktop window.

Connecting these paths must not copy an opened document owner into the desktop's
candidate snapshots. It must also distinguish the authoritative editor state
from the last presented observation: a failed presentation cannot reverse an
acknowledged save. Existing window surface ownership is a local Rust contract,
not authenticated cross-task transport.

The native workspace currently has a conservative 61,240-byte emitted entry
bound within its 64 KiB stack. Existing application tasks use an 8 KiB user
stack and a separately constrained executable profile. Moving the workspace
into that profile or adding frame buffers to its stack is not justified by the
earlier proofs. First establish the ownership and publication composition with
the real portable models and compositor, then qualify native admission.

## Decision

### Add an opt-in external editor window

Gate the workspace extension behind `native_editor_bridge`. Add a distinct
`native_editor` window content kind. Its copyable desktop state contains only
the window's ordinary geometry, lifecycle and an observation descriptor. It
contains no document text, rendered pixels, opened editor, pending snapshot,
save request allocator or document-service authority. The existing scratch
Editor Lab keeps its public memory-only behavior and Save-unavailable feedback.

Retain the actual `interactive_editor_session` as one non-copyable application
owner outside the desktop. Neither a copied observation nor a desktop candidate
can reconstruct, replace or fork that owner. The document service remains the
authority for document operations; the bridge neither manufactures service
replies nor treats presentation as a save acknowledgement.

The bridge retains immutable observations in a sidecar outside the desktop
snapshot. An observation contains validated ANUI data and the corresponding
owned rendered pixels. Its session sequence, local text revision, acknowledged
store version, pending request/revision, dirty state, save outcome, selection,
history and feedback remain distinct meanings. Its public descriptor identifies
that exact observation, not the editor's current mutable state.

ANUI remains an experimental observation encoding. Its fields and rendered
pixels do not prove truthful application behavior or confer authority. This
checkpoint adds no stable wire ABI, kernel parser, native route, filesystem
format or private-document policy.

### Bind local authority independently of copied fields

Trusted local setup binds one producer to an exact desktop instance, granted
client endpoint, native editor window incarnation and editor binding epoch.
The desktop retains mutable binding state so withdrawal or replacement can
invalidate an earlier owner even when a window slot or public identifiers are
reused. Binding and freshness counters do not wrap.

Producer capabilities and action/publication tickets are privately branded,
move-only local values. They cannot be manufactured from a descriptor, ANUI
record, window ID, frame ID, numeric generation or digest. Every operation
checks the live endpoint, grants, exact window and retained binding. Equal
public identifiers from another local instance must not authorize access.

These brands enforce the portable API's local ownership discipline. They are
not native task authentication, serialized capabilities, cryptographic identity
or permission to accept a sender's claimed origin. A future native adapter must
derive the corresponding binding from installed task/channel authority.

### Make disclosure explicit

The first policy is explicitly provisioned `public_fixture` content. Trusted
setup selects that policy; an application cannot expand disclosure by declaring
its text public in an incoming record. The current desktop observe grant covers
the desktop's public state, so this proof does not claim private per-window
filtering or confidentiality from authorized desktop observers.

Observation requires the live desktop, a currently authorized observer endpoint
and the exact window. The bridge resolves the currently committed descriptor
against its retained sidecar and binding. A copied descriptor or old window
view alone cannot retrieve text. Human and machine observers receive the same
authorized semantic observation, tied to the same committed presentation.

Diagnostics use bounded, structured errors. Including fixture content in host
test evidence is deliberate disclosure, not a precedent for logging private
documents. Future private content needs separately scoped observation, input,
capture, accessibility and retention policy.

### Publish one complete observation with the desktop transaction

Keep one committed observation and at most one prepared successor. Retain each
accepted observation and its exact rendered storage unchanged through candidate
composition and publication. Full capacity returns backpressure; it does not
overwrite, merge or silently discard the existing pending observation.

A combined move-only publication ticket retains the exact desktop update
ticket and pending sidecar frame. Candidate access borrows that pair together.
The compositor validates the complete surface binding set before presentation,
clips the native editor to its window viewport and preserves desktop-owned
chrome, other windows and the cursor. All rows use the same immutable frame.

The bridge owns the composition of prepare, begin, presentation attestation and
publication. It preflights every sidecar binding, capacity and freshness check
before consuming desktop publication. Once desktop publication succeeds,
installing the already-retained observation is infallible. No observer can
resolve a new descriptor to old text, or a committed descriptor to an unrelated
pending frame. Geometry-only redraws may retain the same observation.

Do not accept the desktop's copyable descriptive `publication` result as
authority to publish a sidecar. Consuming the exact retained desktop ticket is
the publication boundary. Rejected publication returns the complete combined
ticket without losing its immutable frame or changing committed observations.

Cancellation is available only before presentation begins. It returns the pending
frame only after the matching desktop cancellation succeeds; rejection returns
the complete ticket. After external pixels may have changed, cancellation cannot pretend
to restore the prior presentation. Preserve the existing unavailable/withdrawal
discipline rather than expose a mixed state or retry from an uncertain phase.

The editor may advance independently while an older observation is prepared,
including when a durable reply arrives. Cancellation or display failure changes
only presentation state. It never rolls back the editor, its saved baseline,
pending request, document version or close state. A fresh observation is needed
to resynchronize the window with the authoritative application.

### Admit actions once against the live binding

Retain at most one in-flight action ticket. Admission binds the exact granted
client, desktop revision, focused visible native editor window, editor binding,
action sequence and expected presented session sequence. Ordinary editor actions
are unavailable while the launcher owns modal input. A copied action body or
observation supplies no action authority.

Consumption rechecks the live desktop revision, focus, visibility, grants,
window incarnation and binding, then the editor's actual session sequence.
Changing focus away and back does not revive an old ticket. An intervening save
acknowledgement can advance the editor while the displayed observation remains
older; the old action then returns stale state with no application effect and
requires resynchronization. Validate before invoking the session mutation.

Each action is consumed once. Reject duplicate, foreign, stale, withdrawn and
exhausted tickets without editing text or advancing a save request. Human and
machine clients use this same semantic admission path, with their own granted
endpoints and attribution. Keyboard decoding remains an adapter; it cannot
bypass focus or turn ordinary text into desktop shortcuts.

A foreign or unrecognized ticket is returned intact so its rightful bridge can
still consume or cancel it. An exact locally owned ticket rejected by current
state is consumed without effects; its in-flight slot is released and the caller
must resynchronize. Dropping a live ticket is not automatic cancellation.

Legitimate editor feedback, including rejected edits or a pending Save, remains
an application observation. Save initiation reserves the source model's exact
snapshot; only its exact correlated document reply may acknowledge that save.
A packet acknowledgement, action result or display publication cannot do so.

### Close and withdraw without reviving ownership

Generic workspace close is rejected for a live native editor window. The normal
close action invokes the editor's existing `request_close` policy. Clean idle
state may close immediately; dirty, pending or unknown state requires the
explicit consecutive confirmation defined by the session. Intervening session
actions or successful save transitions invalidate that confirmation.

An exact closed observation from the bound editor authorizes coordinated window
removal. Window removal and sidecar retirement follow the same publication
discipline. A pending bridge operation causes explicit backpressure or requires
its permitted cancellation; it is not silently dropped to make room for close.
Closing does not cancel a disk commit or turn an unknown outcome into a negative
save result. A presentation failure after local close cannot reopen the owner.

A separate controller-only terminal teardown prepares ordinary window removal
after the controller asserts that the application route has been withdrawn.
It requires an observe/manage endpoint and uses normal presentation discipline.
Pending removal blocks action consumption; committed removal invalidates the
exact old window, observations and action permits. This assertion is distinct
from editor-confirmed close: it cannot infer saved state, silently confirm
discard, transfer a pending request to a replacement or revive old grants.

Replacement requires a fresh binding. Old producer owners, observation tokens,
action tickets and publication tickets remain stale after retirement and window
slot reuse. Retained immutable pixels must never borrow storage from a destroyed
application. No native task termination or recovery claim follows from this
local invalidation model.

The current owner attachment is permanent. Retaining the original bridge permits
cancel/retry, but a surviving session cannot reattach after desktop destruction.
Fresh-owner replacement is supported; reattachment requires a later explicit
protocol and must not silently reconstruct the old owner from a copied view.

## Qualification and disproof

Use the real workspace transaction model, compositor, interactive editor model
and document-service contract in host tests. This must exercise their composition,
not only compare bridge getters with values written by the same helper.

Required cases include:

- Foreign local instances with equal public fields, insufficient grants, stale
  client/window/binding generations and every nonwrapping freshness boundary.
- One pending observation, missing or mismatched surface bindings, immutable
  candidate rows, cancel-before-begin and rejected cancel/publication with the
  complete ticket retained.
- Observer parity and rejection of stale sidecar reads, descriptor-only reads,
  retired windows and replacement slot reuse.
- Revoke or focus change between action preparation and consumption, focus
  away-and-back, launcher precedence, duplicate actions and identical human/
  machine effects under the same policy.
- Pending save B followed by edit C and a late exact acknowledgement while an
  older observation is prepared. Cancelled or failed presentation must preserve
  the acknowledged B baseline and dirty C; a stale action must have no effect.
- Rejected and unknown saves, continued editing, close confirmation while
  pending/unknown, an intervening action invalidating confirmation, coordinated
  removal and stale work after replacement.
- Literal expected semantic records and independent pixel/geometry assertions
  for content, caret, selection, status, clipping and preserved desktop chrome.
  Reusing one renderer for both expected and actual output is not independent
  glyph or semantic proof.

Keep the default workspace and qualified native profiles unchanged when the
feature is disabled. Record actual host and regression results separately. No
result in this checkpoint claims that three isolated native tasks ran together,
that a native desktop displayed the new window, or that host presentation
attestation proves physical scanout.

The proof fails if a desktop copy forks save ownership, a descriptor grants
content/action authority, stale focus can edit, sidecar and committed frame
diverge, display failure reverses a save, ordinary close bypasses confirmation,
or retirement makes an old capability usable for a replacement.

## Native admission follow-up

The portable gate is qualified on the 2026-10-05 source checkpoint `a8a3c0d`.
The [qualification log](../../notebook/Development/2026-10-05%20Editor%20Workspace%20Bridge%20Log.md)
records adversarial ownership tests, real durable-model/compositor integration,
all 16 required checks, default artifact comparison, QEMU desktop regressions
and an isolated VirtualBox initial-frame comparison. The hosted preview uses
the real bridge but adds no guest execution evidence. Earlier native stack
evidence carries only for the unchanged default executable bytes and layouts.

The next native integration must choose one coordinator for input, window
publication and presentation. The workspace should own desktop physical input
and display composition; the editor should receive only its scoped input and
surface routes; the document service should retain its separate device grant.
Do not keep competing direct-framebuffer owners or introduce document policy
into the kernel to join the components.

Qualify executable size, CPU ownership, complete stack graphs and task/mapping
lifecycle for all participants. The workspace's 64 KiB stack cannot be assumed
to fit the existing 8 KiB application profile. The committed and pending editor
frames each require 18,432 bytes at 128 by 48 RGB8. Two such buffers need an
explicit retained storage layout; neither hidden stack copies nor an assumption
that both fit one existing 32 KiB private arena is acceptable.

Cross-task input needs a defined focus fence for already-delivered actions,
including acknowledgement, drain/cancel ordering and withdrawal. A copied focus
epoch alone cannot prove current focus at a remote application commit. Retain
bounded backpressure, exact copied routes, immutable complete-frame staging and
an explicit failure path when the presenter or editor retires. Qualify the
native bridge first, then compare the same application contract through the
userspace ELF loader. This portable checkpoint is a prerequisite, not a claim
that Antimatter's Electron/React/CodeMirror frontend or runtime is ported.
