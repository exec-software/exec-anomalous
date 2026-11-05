# ADR 0059: Prove ELF editor replacement against the native baseline

- Status: Proposed; bounded implementation and comparison qualified
- Date: 2026-10-04
- Related: ADR 0018, ADR 0045, ADR 0051, ADR 0055, ADR 0056, ADR 0057, ADR 0058

## Context

[ADR 0058](0058-prove-native-editor-replacement-with-a-retained-document-service.md)
qualifies native editor replacement while one RAM document service retains its
committed bytes, execution state and ownership. Three cases distinguish an
unsaved edit, a save discarded before service execution and a committed save
whose reply is lost. Fresh editor owners must recover the actual committed
bytes through new authority while old routes and handles remain revoked.

The user selected a native baseline followed by ELF comparison. Repeat this
qualified lifecycle through the userspace ELF loader before adding filesystem
durability. [ADR 0057](0057-prove-elf-loading-of-the-native-document-contract.md)
establishes the bounded loader and verified zero-tail representation for the
preceding document contract. Container parsing remains separate from execution
personality and document authority.

## Decision

### Inspected image and loader authority

Add an opt-in `elf_relaunch_test` feature, `elf-relaunch` tooling and an ELF
controller around the existing native replacement runtime. Reuse the same
separately linked `anomalous-native-relaunch-client`; do not introduce another
editor or document service implementation. Preserve its full linked ELF and
exact native RX/RW payloads as the comparison control.

The new host artifact path applies the same bounded representation as ADR 0057:
verify all 4,224 native RW file bytes are zero, retain a 128-byte zero RW file
prefix and preserve the original 4,224-byte virtual memory extent. Elide exactly
4,096 verified-zero file bytes. Independently reconstruct and compare the full
virtual writable image before accepting the deployment. Keep the generic
compactor's exact file-byte preservation rule and every older artifact profile's
acceptance checks unchanged.

Preserve the existing safe ELF parser, two-PT_LOAD subset, 96-byte `ANEL` plan,
fixed virtual geometry and 32 KiB loader RX limit. Artifact inspection must
measure the actual deployment and loader sizes; this decision does not widen
limits to make the fixture fit. No new arena, startup, task, channel or authority
segment is admitted. There is no interpreter, relocation, dynamic linking, TLS
or foreign syscall policy.

Two independently owned ring-3 loader tasks receive the complete immutable
compact deployment. They parse the ELF headers/program table and validate
extents and padding; payload bytes remain opaque to the parser. Only the
controller-designated current loader role may offer one copied inert plan.
The kernel checks exact encoding and equality with the independent inspected
manifest without parsing ELF. Wrong role, failed copy, short/corrupt plan,
altered geometry and duplicate offers fail without creating application tasks.
A valid offer must still succeed after the negative probes.

Retire both loader tasks, contexts, roots, CPU images and all fifty owned frames
in ten leases before the unchanged 21-record native baseline and any replacement
scenario. Only the admitted inert plan survives this loader phase. Its success
confers neither execution authority nor a document handle.

### Adjacent native and ELF lifecycle runs

Run scenarios zero through two in the qualified native order, each immediately
followed by its ELF counterpart. Route zero uses the original native spans;
route one uses the accepted ELF RX span and 128-byte RW prefix, with the omitted
writable tail supplied by zero fill. Both use the native `int 0xf4` personality.

Every initial editor, retained service and replacement construction attempt
must use its selected route's image. This includes the failed population attempt,
the committed but unadmitted cancellation probe and the final replacement;
replacement must not silently fall back to native spans on the ELF route.
Before trusted startup overlay, check initialized bytes and the full writable
pages, private stack and arena zero state. The 128-byte `ANDS` descriptor is
still built from retained task/channel identities and copied at `0x801000`,
inside the ELF route's zero-filled tail. It cannot select its own authority.

Reuse one feature-private retained runtime storage owner and the existing
maintenance path. The service remains the current task while IF is clear,
handler state is unpublished and execution is on the original bootstrap stack.
Retire only the editor, preserve the service's complete private regions, saved
frame and CPU image, and compare exact task/circle/root/stack ownership at all
eight maintenance checkpoints. Repeat the real partial-clear retry, failed
population cancellation and committed-but-unadmitted cancellation. Fresh task
and circle generations must advance without wrapping; skipped generations
from cancelled candidates are expected.

The service receives checked successor metadata and changes document authority
in userspace. Yield and exit must both fail before the exact replacement receipt
is acknowledged. No document record supplies its own authenticated task binding.
The kernel continues to enforce copying, ownership, scheduling and lifecycle;
it does not interpret document operations or decide which save committed.

Each live pair retains the four-region native geometry and owns 66 frames
(48 data, 18 tables) in twelve leases. Destructive editor retirement reaches a
33-frame, six-lease service-only state before successful replacement returns
to 66 frames and twelve leases. Runs and loader phases are sequential; no
capacity increase or second simultaneous runtime owner is introduced.

## Independent comparison and acceptance

Both routes must independently satisfy the known native byte and semantic
checkpoints. Reuse the explicit manual document-wire construction and surface
expectations from the qualified native oracle, without calling the guest store
or save-state implementation. Matching two incorrect routes is insufficient.
Require exact maintenance/resource counters, fresh authority, commit versions,
stale-handle rejection, transport sequence resets, packet hashes and semantic
surface hashes. Only individually bounded timer ticks may differ.

Require the original three recovery results: the acknowledged snapshot after
unsaved editor loss, the same snapshot after a queued save is discarded, and
the newer committed snapshot after its reply is lost. Keep the two fault sites
distinct and inspect the real store observations. Compare complete rendered
frame bytes between adjacent routes before publishing the ELF result; retain
outside-patch preservation and whole-frame host captures. Pixel expectations
reuse the qualified renderer and do not constitute a second font implementation.

The transcript is exactly 222 ordered records: 21 unchanged native baseline
records, readiness and loader receipts, six maintenance records, six scenario
records, 48 surface hashes, 138 document packet hashes and final success. Route
and editor-incarnation labels are mandatory. Reject missing, duplicated,
reordered, extra, malformed or changed fields and records, including identical
wrong mutations on both routes. Capture readiness requires the complete proof.

The six document runs execute eighteen task lifetimes across six retained
services and six editor replacements, with six additional committed but
unstarted candidates cancelled. Four editor faults are contained and 589,824
arena bytes are freshly zeroed. The two loader tasks and four original baseline
task lifetimes are separate, giving twenty-four executed task lifetimes in the
complete proof; cancelled candidates are not counted as executed tasks.

Host artifact tests must reject nonzero elided bytes, changed image geometry or
RX bytes, failed expanded-image equality, malformed deployment, stale wrappers
and exceeded loader bounds. Preserve the native 122-record verifier. Feature
selection must keep the new comparison below Linux/veil/shared and existing
higher-priority diagnostic profiles; all-features retains the shared-veil proof.
The ordinary and workspace images must exclude the new profile.

Retain exact final kernel, client and loader binaries, disassembly, deployment
and plan. Review all admitted indirect targets, stack bounds, CPU instructions,
fault entry and nonlocal maintenance continuation. Reuse earlier emitted
certificates only after complete relevant binary equality is established.
Qualify TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with separate captures, all
required repository checks, prior ABI/runtime/document/replacement regressions
and restored development images. Serialize builds and QEMU under recorded
memory/OOM limits. Final counts, hashes and measurements belong in the
[ELF Relaunch Log](../../notebook/Development/2026-10-04%20ELF%20Relaunch%20Log.md)
only after the corresponding checks pass.

## Bounded qualification

All 52 qualification checks and two development-image restorations passed with
369 frozen source files unchanged. The 222-record proof, including 48 surfaces
and 138 document packets, passed TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with
matching captures. All 99 lifecycle, surface and packet records per route also
matched the qualified native baseline in all six backend/size runs, excluding
only the event prefix, route label and individually bounded timer counts.

Each complete proof executed eighteen document task lifetimes across six
retained services and six editor replacements. Six committed but unstarted candidates
were cancelled, four editor faults were contained, and 589,824 arena bytes were
freshly zeroed. Host checks passed 491 kernel tests for each replacement profile,
531 all-features kernel tests and 221 xtask tests. Exact emitted kernel, client
and loader reviews closed all admitted control paths within retained stack and
CPU bounds. Prior baseline blobs, captures and ordinary/workspace images remain
unchanged; detailed artifact identities and memory receipts are retained in the
[ELF Relaunch Log](../../notebook/Development/2026-10-04%20ELF%20Relaunch%20Log.md).

## Limits and follow-up

This is a fixed ELF fixture using native execution and service policy, not
arbitrary ELF admission, Linux compatibility, a stable ABI or a full Antimatter/
Electron port. Each native/ELF run starts a separate RAM store; route comparison
does not provide cross-service persistence. Saved bytes survive editor
replacement only while that exact service remains alive. Service destruction
or machine restart loses the store.

After this comparison, the next bounded document gate is scoped filesystem
persistence with explicit durability and failure semantics. Service restart,
concurrent editors, general process management, Windows/macOS personalities,
Ny runtime adoption, HAG and Displacer retain separate qualification gates.
