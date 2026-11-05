---
title: "Read-only File Service Log"
date: 2026-10-11
tags: [linux, elf, userspace, file-service, descriptors, namespace, qualification]
status: qualified-host-foundation-native-integration-pending
---

# Read-only File Service Log

This increment implements a reusable portable userspace file-service foundation
for the existing Linux Antimatter/Electron target. It follows the authenticated
live pathname observed under
[ADR 0083](../../docs/adr/0083-observe-the-loaders-live-pathname-before-file-service.md)
and defines its separate host qualification under
[ADR 0084](../../docs/adr/0084-model-read-only-userspace-file-services.md).
The earlier guest remains the three-reply live-path proof; this work does not
return a fourth result, import libc into the guest or launch Antimatter.

## Reusable userspace state

The opt-in `read-only-files` feature supplies a safe, allocation-free model.
It borrows immutable object/version bytes and an explicitly complete namespace
subtree. Exact canonical byte names in that subtree resolve to a held file or
absence; unsupported syntax and other scopes remain distinct errors. File and
directory ambiguity is rejected. It supplies no symlink, normalization or
general directory-open behavior. Borrowed paths have no fixed global length
or depth ceiling; external traversal and copy work still require accounting.

A service belongs to one externally authenticated task incarnation and one
fresh service incarnation. Its supplied descriptor capacity and first integer
reserve a local table range without claiming standard streams exist. Internal
generation-bearing handles distinguish closed/reopened slots and replacement
services; a reused Linux integer alone cannot reject stale caller intent.
Open descriptors retain the actual immutable bytes across namespace replacement.

Open, bounded sequential read and close prepare all fallible work before a
synchronous publication callback. Confirmed publication commits local state;
known rejection changes no descriptor or offset. Unknown publication, callback
unwinding and authenticated retirement fence the service and revoke its local
handles. A last-committed sequence watermark rejects older completed operations;
it is not a replay ledger. Exact retries and completed error replies need
separate authenticated adapter accounting.

The callback is a trusted contract, not proof of native atomic copyout. The
future adapter must reconcile publication before processing a fresh caller
invocation, contain the caller if the veil fails after delivery, and stage
bytes and result together. This host foundation uses complete borrowed byte
slices. Native reuse requires either a separately admitted immutable mapping
or an authenticated chunk-readable source backend preserving these state
transitions; the current native 8 KiB data profile cannot contain this file.

## Configured real-file proof

The selected host libc snapshot contains 2,190,608 bytes with SHA-256
`85e64f97e348786a8fb4d9f3d52fec289e2fb86bba20f0731dfe61990525e0f7`.
It was read from `/lib/x86_64-linux-gnu/libc.so.6`, resolving to
`/usr/lib/x86_64-linux-gnu/libc.so.6`, and copied into this task's evidence
before qualification. No file is executed.

The host proof declares `/lib/x86_64-linux-gnu` as a complete test namespace
and binds the base libc pathname to those exact held bytes. The previously
observed `/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libc.so.6` is absent
by that explicit test configuration. This is not a host-filesystem inference
or a change to the guest's existing v3 preload/cache policy.

Qualification reconstructed all 2,190,608 input bytes through each of two
independent descriptor tables. Both retained output streams match the pinned
input exactly. The independent verifier recomputed all 2,739 read publications:
2,737 committed, one rejected without offset change and one unknown outcome
that fenced its separate service. Three committed reads delivered zero bytes
(zero-count and two EOFs); 2,733 were nonempty short reads under explicit budgets.
The owners ended their read streams at committed sequences 539 and 2,200.
Close/reopen, stale handles and independent offsets were checked separately.
The host input budget is 8 MiB; it is neither a native file size ceiling nor an
expansion of guest provisioning authority.

## Qualification

The final focused scope passed all eight jobs: sixteen protocol tests (fourteen
new file-service groups), all 359 xtask tests (seven new host-proof groups),
host and freestanding strict lint, the held-file proof, and guest rebuild/capture.
The first attempt stopped on a collapsible-if lint error after its tests passed;
its logs and memory history remain retained. The corrected final source is
`source-freeze-v2.json`, with 484 non-Markdown files. Source review and the
independent host proof audit passed. The complete required/affected regression
matrix passed in `qualification-v2`. Final physical, baseline and evidence
checks passed. Builds and VMs ran serially under the unchanged memory guard. The evidence directory is
an external validation directory (not distributed).
The preceding live-path evidence remains unchanged in its own directory.

The first 29-command regression attempt passed its first eighteen commands,
including all sixteen required targets, then the unchanged memory guard stopped
`owned-source-smoke` on host-wide full-stall pressure: 19.7656% over two seconds
against the 10% cutoff. Its own scope measured about 0.0315% over that interval.
These observations do not attribute the host pressure to a particular process.
No OOM kill or hard-limit event was recorded. The interrupted attempt and all
samples remain retained. The fresh complete `qualification-v2` run passed all
29 commands under the same guard and quiet-start requirement. Its peak was
3,215,757,312 bytes, with at least 23,407,460,352 host bytes available and no
pressure abort or OOM event. The first attempt had 1,001 memory-high events,
a 4,297,043,968-byte peak and up to 143,851,520 logged swap bytes in its own
scope; these are retained observations, not an OOM kill or a claim of zero
memory pressure across the entire turn.

The independent host receipt is `host-proof-review-v2.json`, SHA-256
`fcb8be1a59df1c0668d9d5249ec03794cfda0ceb269a217e4d15f7a9a7dd7207`.
It binds both streams, the held input, JSON transaction report, exact guarded
command, final source inventory and independent auditor. This is host evidence,
not guest file-service execution.

## Final guest regression and evidence

Fresh TCG and KVM runs at 96/320 MiB passed the unchanged live-path guest proof:
four raw invocations, three committed replies, the 55-byte NUL-terminated live
hwcaps pathname, unchanged complete terminal caller frames and full retirement.
Missing-source, wrong-length and one-byte-corruption provisioning were rejected.
VirtualBox reported `application_cpu_amd_error_pointers` before userspace and
powered off; it remains unqualified for this proof.

The rebuilt kernel/native loaded bytes, layout and permissions equal the
preceding live-path checkpoint exactly; only debug sections changed. The prior
emitted stack bounds therefore still apply. All five complete 16 MiB baseline
images are byte-identical. All 484 frozen source hashes remained unchanged.

The final manifest is `final-evidence-v2.json`, SHA-256
`ae2214e4094c972e591d0cd344be7b34fbfc3bb0d7ed8cabf17bb0060704a4b0`.
The passed regression/memory receipt is `final-audit-v2/receipt.json`, SHA-256
`891b2cc5c5cb12c662a6e1486dc4c30413f2def9c5ab4c53314e618f2a865a0d`.
It selects 44 successful jobs across six scopes and seven artifact sets, while
preserving the focused lint failure, pressure-aborted regression and unsupported
VirtualBox attempt. The separate host receipt above qualifies file-service
behavior; the guest receipt qualifies the existing loader regression only.

The first final-audit invocation failed on a copied path assumption: it looked
for `baseline/` rather than this checkpoint's retained `baseline-before/`.
The original auditor, manifest and failed audit remain intact. The reviewed
`audit-baseline-v2.py` changes only that directory name, with no weakened
acceptance rule; the final manifest binds the replacement, independent path
review and failed attempt. This audit-only correction required no code rebuild.
Earlier source-review document hashes describe their pending snapshots; the
final documentation is qualified against the completed receipts here.

Across all retained guarded attempts, peak scope memory was 4,297,043,968 bytes
(about 4.002 GiB), and minimum host available memory was 14,940,778,496 bytes.
The final audit found no OOM-counter increase or accessible OOM-kill evidence.
This does not imply zero pressure: the explicit pressure abort and memory-high
events above remain part of the result. Accessible journals, retained units
and one-second samples cannot establish unobservable historical events or
reconstruct every intersample pressure fluctuation.

## Native integration next

The libc snapshot needs 535 source pages, while its LOAD mappings would need
545 user pages. The present source profile admits only 512 KiB and the exact
`true + interpreter + used stack` snapshot. Keep that launch contract intact
and add a separate pinned file owner with a reviewed local budget. Source reads
need access through the retained live physical alias; provisioning-time aliases
cannot simply be installed while user roots run.

The current loader gate admits request/read/complete operations, not the
existing staged copyout service. Its completion rejects stage tokens. Real
guest reads need full destination preflight, a retained stage, reply
revalidation, caller copyout and committed result under the same pending owner.
File names, descriptor allocation and offsets remain userspace policy.

The preceding kernel bootstrap certificate has only 2,720 bytes beyond its
separate 16 KiB reserve. Increasing returned full-frame arrays to report more
syscalls needs fresh emitted review or a retained observation design. None of
these source, copyout, stack or mapping gates is closed by this host model.
Dependency loading, libc startup, a real Linux exit and the larger Electron
process/display environment remain open.
