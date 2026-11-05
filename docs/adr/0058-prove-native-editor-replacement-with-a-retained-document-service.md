# ADR 0058: Prove native editor replacement with a retained document service

- Status: Proposed; bounded implementation qualified
- Date: 2026-10-04
- Related: ADR 0018, ADR 0045, ADR 0046, ADR 0048, ADR 0055, ADR 0056, ADR 0057

## Context

[ADR 0056](0056-prove-task-bound-native-document-open-and-save.md) qualifies
bounded native document open/save and uncertain outcomes after service loss.
[ADR 0057](0057-prove-elf-loading-of-the-native-document-contract.md) qualifies
the same contract through an inspected ELF fixture. Both destroy the editor
and RAM service together at the end of each scenario. Close/reopen within one
pair does not prove that saved bytes survive destruction of an editor owner.

The next native increment keeps the RAM document service alive while retiring
and replacing its editor. Accepted
[ADR 0018](0018-prove-bounded-vessel-supervision-and-restart.md) establishes the
essential lifecycle rule: destructive replacement occurs on the original
kernel stack after a checked checkpoint, with fresh task and circle identities.
A terminal task is never reset to ready.

## Decision

Add an opt-in `native_relaunch_test` profile, `native-relaunch` tooling and a
separately inspected `anomalous-native-relaunch-client` fixture. Retain the
native `int 0xf4` personality, bounded document records and four-region native
owner geometry. Each scenario has one service lifetime and two executed editor
lifetimes, plus a committed but unadmitted editor cancellation probe, with at
most two tasks live at once. The preceding native and ELF profiles keep their
acceptance rules.

### Three observable outcomes

Every first editor opens the seed at handle generation one/version one, saves
an exact changed snapshot and receives version two, then makes distinct newer
edits. Run these fixed cases in order:

1. The first editor exits with those newer edits unsaved. Its replacement opens
   the acknowledged snapshot at version two; the unsaved edits do not become
   saved.
2. The first editor submits the newer edits as a second save, then faults while
   that request remains queued, before the service receives it. Route closure
   discards the request. The replacement opens the acknowledged snapshot at
   version two.
3. The service commits the second save as version three and queues its semantic
   reply. A separate input handshake lets the editor fault before consuming
   that reply. Route closure discards the reply. The replacement opens the
   newer committed bytes at version three.

The replacement obtains a fresh document handle through an authenticated open
reply at handle generation two, rejects the old handle, then performs another
edit/save/read/close/reopen/close sequence. Reopen uses handle generation three.
The fresh save request sequence begins at one against the recovered version.
The replacement does not inherit the old editor's pending snapshot, undo
history, saved acknowledgement or private memory. A fresh open reports the
surviving store's current state; it does not retroactively acknowledge a lost
request to a destroyed editor.

### Retained service and destructive owner replacement

Close and clear all old task-bound routes before withdrawing the old editor's
context or storage. Prepared channel tokens, endpoint incarnations and old
task/circle identities cannot authorize the replacement. Fresh route owners
start their own bounded sequences; numerical sequence reuse does not restore
the old binding.

The service reaches an explicit maintenance gate with a captured frame and
CPU image. It remains the scheduler's exact current task throughout the
checkpoint. No new parked state or general wake operation is introduced.
Require no runnable peer or pending dispatch reservation, IF clear, NMI masked,
and unpublished runtime/dispatch pointers before returning to the original
kernel stack. The new maintenance path is separate from the existing terminal
bootstrap-restoration and whole-group teardown contracts.

Only after kernel CR3, bootstrap TSS RSP0 and the original kernel stack are
restored may the controller withdraw the terminal editor context, unregister
its root, clear and release its mappings and transition stack, and destroy its
task and circle. Retain the service's exact task, circle, root, mappings,
transition stack, saved frame, private bytes and eager CPU image. Do not finish
and restart the whole CPU session or rewrite the service's document store.

Construct the replacement through retained preparation ownership after the old
owner is gone. Check strictly advancing task and circle generations, zeroed
data/stack/arena storage before startup overlay, separate root/stack ownership
and a clean editor CPU image. Validate the selected service frame/root/TSS and
all new bindings before publishing another single-use user-session continuation.
No allocator alias or checked-copy span crosses that publication. Cancellation
of a committed but unadmitted replacement may consume a task generation;
advancement need not be exactly one.

The pair owns 66 frames and twelve leases. With only the retained service it
owns 33 frames (24 data and nine page-table frames) and six leases. Replacing
the editor restores the 66-frame/twelve-lease live pair; final teardown returns
the original accounting. An injected release failure must retain the old
editor's remaining ownership for retry. An injected replacement population
callback failure must retain its partial prepared owner until cancellation and
retry. A separate publication-preflight failure cancels a committed but
unadmitted replacement before another attempt. This does not roll back an
already admitted scheduler task. None of these paths may clear, replace or
dispatch the service, publish an incomplete editor, or make ambiguous storage
allocatable.

### Userspace document authority

Document records and copied startup fields remain correlation data. A trusted
route observation supplies the replacement peer identity separately from any
document request. The service-only maintenance and route-refresh gates return
a checked copy of the replacement editor's existing 128-byte `ANDS` descriptor;
the ordinary descriptor format remains unchanged. A separate acknowledgement
must name the exact current replacement task generation before new client work
is admitted. Wrong-role, wrong-generation and repeated acknowledgements fail
without advancing that lifecycle. Userspace policy checks the expected previous
client before changing its client binding, invalidates the previous open handle,
and retains the document identity, committed bytes, version and nonwrapping
handle history.
Wrong or stale replacement observations fail before mutation. Old client
authority and old document handles remain rejected after a successful change.

The kernel provides task/channel provenance, lifecycle, copying and containment.
It does not decode document operations, choose a saved baseline, recover a
snapshot, or transfer document policy into the kernel. The service continues
from its retained userspace state and performs the policy transition itself.

## Acceptance and evidence

Preserve the original 21-record native baseline, then require an exact ordered
122-record relaunch transcript, including 24 surfaces and 69 document packet
hashes. Independently construct expected document wire bytes and
surface checkpoints without calling the guest store or save-state methods.
Verify both initial-editor and replacement-editor sequences, committed versions,
fresh handle generations, stale-authority rejection, discarded queued work,
exact resource transitions and final pixels. Timer values alone may vary
within explicit bounds. Reject missing, extra, duplicate, reordered, malformed
or changed fields and records.

Observe real committed service bytes and version before the post-commit fault
boundary. Inspect the emitted code so an optimized-away store cannot satisfy
the proof. Across maintenance, compare the full retained service private
regions, saved frame and CPU image, together with exact owner identities and
root/stack bindings. Require actual fresh zero observations and retry evidence,
not solely a final success boolean. Emit no private text, pointers, physical
addresses, saved register values or arbitrary user-provided serial content.

Host tests cover the userspace authority change, descriptor validation,
generation exhaustion and strict evidence mutations. Existing launch/channel
tests remain mandatory. The native proof exercises the owner-specific release,
preparation and publication-preflight failures and retries described above.
Retain and inspect the exact kernel/client binaries, all admitted indirect
targets and bootstrap/gate/fault/timer/client stack bounds.
The eight-page RX, two-page data, guarded 32 KiB arena, two-page user stack and
Threshold staging limits remain mandatory.

Qualify TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with separate captures, all
required repository checks, prior ABI/runtime/document regressions, normal-image
exclusion and restored development images. Serialize builds and QEMU under
recorded memory/OOM limits. Record final counts, hashes and measurements only
after qualification; implementation work does not establish completion.

Bounded qualification is complete: all 50 matrix checks and two development-image
restorations passed, including TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with
matching captures. The strict oracle verified 122 records, 24 surfaces and 69
document packets. Within the new profile, nine executed task lifetimes span
three retained services and three editor replacements; three committed but
unstarted candidates are cancelled separately. Two editor faults are contained
and 294,912 arena bytes are verified freshly zeroed. Earlier artifact payloads
and captures, plus normal/workspace development images, remain unchanged.
The [Native Relaunch Log](../../notebook/Development/2026-10-04%20Native%20Relaunch%20Log.md)
retains exact binaries, host counts, emitted-code reviews, failed attempts and
measured memory receipts. No OOM or hard-limit event occurred; recorded soft-limit
throttling is distinct from those failures.

## Limits and follow-up

This is a fixed native Rust client and one bounded RAM document service, not
full Antimatter/Electron, a general process manager, an authenticated user
identity service or a stable native ABI. There is no concurrent editor, general
service discovery, durable file, filesystem namespace or restart of the service.
Saved bytes survive editor replacement only while this exact service remains
alive. Service destruction or machine restart still loses the store.

With this native proof qualified, the next bounded gate repeats the same
retained-service editor replacement through the userspace ELF loader against
this native baseline. Both routes must independently satisfy the document,
lifecycle and pixel oracle while retaining the native `int 0xf4` personality;
ELF parsing itself grants no execution or document authority.

Scoped filesystem persistence with explicit durability and failure semantics
follows that comparison. Windows/macOS personalities, Ny runtime adoption, HAG
and Displacer remain separate qualification work.
