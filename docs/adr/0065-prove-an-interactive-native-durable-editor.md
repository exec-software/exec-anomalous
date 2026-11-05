# ADR 0065: Prove an interactive native durable editor

- Status: Proposed; bounded native implementation qualified
- Date: 2026-10-05
- Related: ADR 0045, ADR 0048, ADR 0050, ADR 0056, ADR 0063, ADR 0064

## Context

The native document proofs establish owned userspace execution, exact snapshot
acknowledgements and a bounded durable store. Their editing traces are scripted.
Editor Lab adds physical keyboard editing and a canonical public scratch model,
but shares the workspace vessel and deliberately has no save authority. Neither
is an interactive editor connected to an independently owned durable service.

The next native gate combines those capabilities without embedding a save owner
in a copyable desktop snapshot or introducing document policy into the kernel.
It remains a bounded Rust reference for the native Antimatter direction, not the
Electron, React, CodeMirror or Ny runtime port. The native result must precede a
comparison of this same contract through the userspace ELF loader.

## Decision

### Isolate the editor and durable service

Add the opt-in `native_interactive_editor_test` kernel profile and build the
existing native durable client with `interactive_proof`. One task owns the
interactive editor; a separate task owns the durable document service. Retain
the existing native CPU profile, checked copies, owner-bound mappings and
cleanup discipline. The two four-region owners use the existing 32 KiB private
arenas, with 66 retained frames and 12 leases. Final executable and stack limits
require artifact-specific inspection, not inference from an earlier build.

The physical keyboard route and display staging grant belong only to the editor.
The sealed, bounded device grant belongs only to the service. Guessing the other
task's numeric handle must not grant its operations. The service receives the
actual client task identity separately from decoded document bytes. Copied
startup fields describe installed bindings; modifying them cannot create a
task, mapping, channel, disk extent or right.

Keep four independent opaque copied-packet routes under ADR 0050's one-packet
ownership and acknowledgement rules. Route 0 carries the editor's UI audit to
the service; route 2 carries document requests; route 3 carries mount readiness
and document replies. The reverse route 1 is reserved in this profile. Physical
input is delivered by the dedicated checked input gate, not forged as a
service-authored keyboard record. Kernel transport remains unaware of document
operations, editor state, audit meanings and save policy.

### Retain one save owner while editing

`interactive_editor_session` owns exactly one
`repeated_opened_editor_state`. Both remain non-copyable and non-cloneable, with
read-only model access. Backspace and Delete reuse staged local edits, retaining
the original selection for undo. Caret movement and selection never overwrite
the saved baseline, request counter or pending snapshot. Earlier editor types
retain their original history behavior and layouts.

Each successful session event advances a separate nonwrapping sequence. Text
revision still advances only on changed text or undo; selection, feedback and
save outcomes are observable without pretending to be text edits. Legitimate
edit rejection publishes structured feedback while preserving text, selection,
history, saved bytes and pending bytes. Boundary deletion and content no-ops
preserve the existing undo entry. Input reset changes feedback, not text or
document authority.

Beginning a save reserves and returns the exact snapshot before transport
submission. A newer edit cannot replace it. Only an authenticated reply matching
document, open generation, request, base version, submitted revision, exact bytes
and next version advances the saved baseline. Acknowledging B while C is current
saves B and leaves C dirty. A subsequent save remains blocked while a request is
pending. Unknown outcome retains the snapshot and forces unsynchronized/dirty
state even if an undo restores the last acknowledged bytes. A negative reply,
especially `stale_request`, cannot prove an uncertain earlier attempt failed.

Save-method errors are atomic, including malformed replies and exhausted
counters. A session cannot wrap its sequence to admit another event. A caller
must retain uncertainty and fail or retire explicitly if it cannot record a
result; it must not manufacture a successful display or durable acknowledgement.

A copied `interactive_editor_view` is an immutable observation of current text,
selection, text revision, session sequence, saved version, undo depth, dirty
state, save outcome, pending request/revision, feedback and close state. It
contains no save authority. Local session state is authoritative before its
observation is rendered. A failed display publication does not reverse an
accepted edit or an actual durable acknowledgement; the client must retry or
report the unpublished observation. This differs deliberately from Editor Lab's
copyable desktop candidate transaction.

Close while clean and idle succeeds immediately. Dirty, pending or unknown
state requires an explicit second close with no intervening processed local
action or successful save transition. Even a rejected edit, no-op or input
reset clears that confirmation. A malformed save reply or rejected save-method
call is wholly atomic and does not change it. Closing abandons local ownership;
it is not a service close acknowledgement, save cancellation or grant revocation.
The caller still tears down channels and authority. Closed sessions reject
mutation and retain readable state until dropped; logical retirement does not
claim secure erasure of copies.

### Keep startup and observations explicit

The new version-one `ANDI` startup descriptor is exactly 192 bytes. It contains
role, phase, arena span, four route handles, client identity, provisioned
volume/object identity, service-only device handle and proof mode. Reserved bytes
are zero. The separate host `ANDF` fixture is exactly 64 bytes: version and size,
phase 0 for disposable provisioning or 1 for recovery, proof mode 0 through 2,
and a zero tail. Neither format contains editor text or an expected recovery
answer. Phase and mode select the proof; they do not authorize disk operations.

The service obtains content and the fresh mount/document epoch through the
existing userspace durable mount. The editor waits for the authenticated mount
readiness exchange before opening. The kernel does not derive a document epoch
by parsing storage. Formatting remains explicit and limited to the provisioned
disposable extent; recovery must mount the retained data disk without formatting.

While route 0 is live, the editor encodes an exact 128-byte version-one `ANUI`
audit for each published session state. It includes the independent session,
text and store counters, pending
request and revision, selection, undo, dirty/save/feedback/close fields and
zero-padded bounded text. Decoding validates exact length, reserved bytes,
alphabet, ranges and internal pending-state consistency. Sequence and content
fields are correlation data, not proof of authority or truthful rendering.

ANUI text and its serial/capture evidence are explicitly provisioned public
fixture content. They do not establish a private-document disclosure policy or
permit adding arbitrary document bytes to public desktop snapshots. Future
workspace and machine-access bridges require exact owner-bound observation and
action routes. People and machines must consume the same authorized semantic
state; neither may infer save success from pixels alone.

### Present complete interactive frames

The userspace renderer derives a 128 by 48 RGB8 image from one immutable view,
including caret/selection, save state, versions and feedback. The editor scales
it by three into a 384 by 144 rectangle at `(32, 32)`. Gate 7 accepts only the
next complete checked row into a private kernel stage. Gate 16 requires every
row before committing and reading back the complete rectangle, checking the
outside framebuffer remains unchanged, clearing the stage and advancing the
frame sequence. There is no user framebuffer mapping or document parser in
this mechanism.

The presentation helper first validates the exact retained task, admitted root,
selected TSS stack and live high-alias stack extent. With IF clear and the user
CPU image already captured, it temporarily selects bootstrap CR3 only for the
integer framebuffer/MMIO callback. No user copy, dispatch or ownership change
is admitted there. It verifies the callback preserved the stack/root contract,
then restores and revalidates the exact task root before any userspace return.
Contract or restore failure stops execution while retaining ownership; it does
not retry from an uncertain root. The final emitted review must cover this
temporary mapping transition and its CPU/stack bounds.

Packet acknowledgement, service save acknowledgement, session sequence and
display completion remain different events. An incomplete or rejected stage
cannot publish a frame. A committed frame does not itself prove a disk write.
The direct editor display grant remains usable after the service exits and all
packet routes close, allowing the surviving editor to show newer unsaved text
and an unknown outcome. This is a fixed-region proof, not workspace integration,
general window admission, scanout completion or tear-free presentation.

### Yield without an unbounded idle spin

Gate 15 copies existing physical-input records only to the exact selected
editor's preflighted private destination. Failure must not consume queued input
or alter a rejected destination. Keyboard policy, held-key suppression,
modifier state and loss handling remain userspace responsibilities.

A valid cooperative yield can sleep only for the editor after its preceding
input read observed empty, with every packet route empty or closed. The kernel
rechecks the actual input queue with interrupts disabled. Only then does the
dedicated idle interval execute `STI; HLT; CLI`. Under the qualified PIT setup,
one timer tick bounds the sleep even if the peer has private work; input can
wake it sooner. No storage or checked-copy operation sleeps this way. On wake,
interrupts are again disabled before selecting a live peer. With a terminal
peer the editor resumes itself. Kernel-origin IRQ paths must not reborrow the
active runtime while its gate owns it. Timer ticks and cooperative switches are
reported separately; this is not a general scheduler wait/wake service.

### Exercise three service outcomes and cold recovery

| Mode | Service behavior | Required editor observation |
| --- | --- | --- |
| 0 | Execute and return ordinary durable save replies | A successful exact reply advances the acknowledged baseline; further editing and saving remain possible |
| 1 | Commit B and retain its reply until an audit matches B's pending request/revision and has a newer current text revision | Pending B remains exact while C is edited; the late B reply leaves C dirty |
| 2 | Commit B, drop its reply and retire the service | Closed routes preserve pending B as unknown; C remains editable and visible through direct presentation |

These are userspace proof controls. Mode 2 is deliberate service retirement,
not another claim about injected device failures; ADR 0063 retains that separate
qualification. Input and new text must come through physical-key delivery, not
startup or a kernel-authored editing script. The service may inspect the public
audit for proof coordination but does not become the editor's state owner.

Cold recovery runs in a separate VM process with fresh tasks and mount authority,
the same disposable data disk and no expected answer in startup. An authenticated
open reports recovered committed bytes/version. It must not retroactively claim
that an old unknown request had a durable receipt: the existing storage model
persists content/version/epoch, not a request-result ledger.

## Qualification and disproof

Bounded native qualification is complete. The eight-boot physical trace checks
62 complete frames, delayed replies, a second save, unknown outcomes, continued
editing, explicit discard and cold recovery without an answer in startup. Live
role/copy/staging probes and host model/codec tests complement the trace. Input
loss and hostile service faults are not newly exercised by this interactive
trace; those need additional cases before a broader claim. Existing workspace
shortcut/loss coverage and native durable-failure proofs remain separate.

Host-predicted audit bytes and recovered disk contents are independent of the
live model. Frame checks compare each complete presented frame; sharing a
renderer does not by itself provide independent glyph or semantic evidence.
Independent glyph/caret assertions supplement that comparison. After service
exit no audit route remains, so hidden metadata is not separately attested even
though complete predicted pixels continue to be checked. See the
[qualification log](../../notebook/Development/2026-10-05%20Interactive%20Durable%20Editor%20Log.md)
for exact artifacts, stack/CPU reviews, all 16 required checks, affected
regressions, VirtualBox scope and zero-OOM measurements.

Native evidence must bind exact binaries, typed inputs, packet sequences,
complete frames, disk-operation boundaries, cold recovery and terminal cleanup.
Reject missing, reordered, extra or mutated evidence. Keep timer/idle variability
explicit rather than treating it as deterministic semantic counts. Inspect
client and kernel emitted stacks, indirect transfers and CPU instructions under
their existing budgets. Re-run required checks and affected native/ELF/workspace
regressions with serialized memory guards; preserve prior qualified receipts.
Qualification remains limited to the recorded artifacts, exercised cases and
resource bounds.

The proof fails if a copied view forks a save owner, pending bytes track newer
edits, negative replies erase uncertainty, physical input reaches the service,
the editor gains disk authority, incomplete rows become visible, peer exit
prevents uncertainty display, idle loses queued input, recovery receives its
answer, or cold mounting revives old grants.

## Following gates

Next, design the qualified native pair's scoped workspace surface/input/semantic
bridge and compare the same document contract through the userspace ELF loader.
General files, chunked text, native names,
project roots, snapshots, backup and redundancy remain the separate filesystem
direction. The 31-byte ASCII line and one-step undo are fixture bounds, not
global editor or path limits. Full Antimatter frontend/runtime and Ny/HAG work
retain their independent adoption gates.
