# ADR 0071: Compose a copied remote editor adapter

- Status: Proposed; copied host composition qualified, native follow-on in ADR 0072
- Date: 2026-10-07
- Related: ADR 0065, ADR 0066, ADR 0069, ADR 0070

## Context

ADR 0070 qualifies three retained native task layouts and their copied routes,
but its small probes do not execute the workspace, editor or document service.
The portable bridge in ADR 0069 still borrows the editor owner directly when
consuming an action or checking a frame. That operation cannot cross private
address spaces. The next prerequisite is an explicit copied application protocol
with sole ownership of the editor session and a workspace receipt boundary.

## Decision

Add opt-in `remote_editor` to the editor contract and `native_editor_remote` to
the workspace/compositor. Keep the existing local bridge usable, with its exact
owner comparison required. Share only private frame binding/publication helpers.
No guest image selects these features in this checkpoint. The protocol below is
an experimental fixture, not a stable Reliquary format, general launch ABI or
Electron port. Existing ANUI, ANDC and disk formats remain unchanged.

### Copy data; authenticate the route separately

Use canonical little-endian messages, never Rust memory layout:

| Packet | Bytes | Contents |
| --- | ---: | --- |
| ANEA v1 | 64 | Connection, action ID, expected ANUI sequence, action tag and two operands |
| ANER v1 | 192 | Observation/completion kind, outcome/error tags, connection and action correlation, then the unchanged 128-byte ANUI |

Require the complete declared size, exact version, zero reserved bytes and
canonical operands. Validate outcome/observation consistency. Public constructors
and decoded packets confer no authority. IDs correlate an already authenticated
route; they do not identify a sender securely.

A desktop controller provisions a non-cloneable remote port with immutable
workspace/editor task and action/observation channel incarnations. Only the
trusted embedding adapter may assert a successful checked receive on that port.
It must derive identity and transport order from the installed kernel route,
not application fields. The portable API can verify supplied correlation but
cannot independently establish that an OS receive happened. Likewise the editor
execution entry point requires its embedding to authenticate the action route.
This is a deliberate trusted userspace integration seam, not a new kernel
security claim. Tests use the existing coordinator's privately branded role
capabilities to model this seam before crossing encoded byte arrays.

The port requires consecutive transport delivery starting at one. A rejected
packet changes neither that expectation nor accepted observations. There is no
packet repair, replay cache, lost-completion retransmission or replacement
handshake in this fixture. The native integration must surface invalid delivery
as a structured route failure and establish terminal retirement before recovery;
it must never infer success or retry an uncertain action with a fresh ID.

### Keep one session and an exact action fence

Only the editor owns an `interactive_editor_session`. The workspace contains
copied observations, two borrowed pixel frames and correlation, with no mutable
shadow editor, save snapshot or document authority. The editor checks the exact
next action ID and counter headroom before effects. Malformed, foreign, duplicate
or future-ID requests have no effects and do not advance the action counter. An
otherwise exact request whose expected editor sequence is stale consumes that
ID and returns a stale completion without changing the session.

Submission requires the current desktop revision, live input grant, focused
visible editor, closed launcher/modal context, no pending publication and the
latest authenticated observation already displayed. It retains origin and exact
action/expected-sequence correlation and installs the existing desktop fence.
Focus, modal and grant changes remain blocked until exact completion. Transport
acknowledgement, timeout, dropped packets and unsolicited observations cannot
release that fence. Receipt draining remains possible while fenced.

A completion is validated in full before changing transport sequence, latest
observation or the fence. Save completion means local snapshot reservation;
only an exact document-service reply can advance the saved baseline. Save B,
edit C, then acknowledge B must leave C dirty. Service loss marks the pending
save unknown and leaves workspace/editor communication and local editing live.
It cannot manufacture a negative or successful save result.

Keep at most one latest validated ANUI. A newer unsolicited observation can
arrive before an older exact completion; that completion still releases its
fence without rolling back the newer observation. Identical same-sequence
observations are idempotent, conflicting bytes are rejected, and closed state
cannot be revived. New actions wait for the latest accepted view to be displayed.

### Preserve publication and frame ownership

Render into caller-owned 18,432-byte slots and retain exclusive borrows. A frame
is admitted only when its ANUI exactly matches the port's latest checked receipt.
A raw decoded ANUI or `from_audit` frame does not supply that receipt authority.
Once admitted, a pending frame remains immutable even if a newer observation
arrives. It may complete publication, after which the latest view must be
published before another action. Rejection/cancellation returns the retained
slot, and successful publication recycles the former displayed slot.

Trusted terminal editor withdrawal requires exact binding and desktop control.
It first invalidates the port and bridge, then releases a matching action fence;
it claims neither save success nor confirmed close. A pending or begun desktop
publication must first be explicitly cancelled or completed so its frame is not
stranded. Ordinary desktop window removal follows retirement. Public packet
fields, elapsed time and document-service loss cannot invoke this recovery rule.

## Qualification

Test canonical bytes independently, reserved/tag/length mutations, replay and
counter boundaries. Compose actual editor, durable store and complete row
compositor through encoded messages. Exercise delayed action, service-reply
interposition, exact stale completion, newer observation before older completion,
foreign binding, malformed data, close and retirement. Check the same policy for
human and machine origins. Assert stable borrowed-slot addresses, immutable
pending/displayed bytes, cancellation recovery and literal glyph/geometry
expectations in addition to comparisons with the existing renderer.

Run host and freestanding warnings-denied checks with the opt-in features, all
sixteen repository-required targets, the existing desktop regression and four
baseline image comparisons. Serialize builds/VMs, keep one build job, enforce
4 GiB memory.high / 6 GiB memory.max / 512 MiB scope swap and monitor host and
ancestor memory pressure. Keep failed attempts in the qualification evidence.

## Remaining native integration

The next slice must select real workspace/editor/service payloads under the
single ADR 0070 CPU/dispatch owner. Workspace alone will own input and display;
editor/service cannot retain the old pair's input/display gate. The service's
interactive proof loop currently reads ANUI on pair route zero to release a
held save reply and observe closure. That assumption is incompatible with the
mixed route zero (workspace actions) and must be adapted explicitly. It must not
gain workspace action authority merely to preserve the old proof script.

Also adapt focused physical input dispatch, role/startup descriptors, checked
receive provenance and document-device roles; retain the mapped two-frame arena
and separately account the framebuffer stage. Qualify final emitted RX/data,
user/kernel stacks and cleanup, since the existing workspace has little stack
headroom and the combined flat kernel image has a fixed boot capacity. Host
compilation and small retained Rust values do not prove those native bounds.
Actual physical editing, a composed native window, delayed save/service loss,
cold recovery and selected VM qualification remain open. This checkpoint does
not establish native Antimatter rendering or a full Electron/React/CodeMirror
runtime.

## Qualification result

Source checkpoint `82ff1d9` passed the focused host and freestanding tests/lints,
all sixteen required repository checks and the existing 512-frame desktop
regression. The four rebuilt normal/workspace boot images are byte-identical
to their starting artifacts; all 436 frozen non-Markdown source/configuration
files remained unchanged. Three serialized guarded scopes completed all 28 jobs
without an observed OOM, pressure abort or failed job. The maximum scope peak
was 3.207 GiB. The
[qualification log](../../notebook/Development/2026-10-07%20Remote%20Editor%20Adapter%20Log.md)
records exact hashes, test cases, memory samples and journal limitations. These
are host adapter and existing-image regression results, not new native payload
execution or an emitted stack bound for the copied adapter.

The subsequent [ADR 0072](0072-integrate-a-native-editor-workspace.md) binds this
checked receive adapter to installed native routes and runs the real three
payloads under one CPU/dispatch owner. Its sixteen-boot TCG/KVM matrix and final
emitted review cover the native integration left open here, including composed
physical editing, delayed save, service loss, cold recovery and cleanup. Its
required checks, regressions and final audit also passed. The host qualification
above remains a separate historical result; full
Electron/React/CodeMirror Antimatter execution remains open.
