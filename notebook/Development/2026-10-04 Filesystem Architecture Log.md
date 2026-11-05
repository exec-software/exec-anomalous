---
title: "Native Filesystem Architecture"
date: 2026-10-04
tags: [filesystem, storage, userspace, antimatter, identity, backup, recovery]
status: architecture_selected
---

# Native filesystem architecture

## User direction and audit

The user requested a filesystem architecture suited to people and AI, with
descriptive names, no native path ceilings, stability, redundancy and easy
backups. Antimatter remains product Milestone 1; this architecture checkpoint
supports its next storage increment rather than making the full filesystem a
prerequisite for the first native application.

Starting revision was `3e8366c` on clean `main`, synchronized with `origin/main`.
The audit found no general filesystem, namespace resolver or runtime block
driver. Threshold reads the boot disk before long mode; it supplies no runtime
application writes. The native document and relaunch clients still instantiate
the RAM `document_store` in their service entry paths.

`crates/anomalous-document-protocol/src/durable.rs` is the separately qualified
ADR 0060 model: five granted 512-byte sectors, explicit blank-only formatting,
volume/object identity, two alternating body/commit pairs, CRC32C, mount epochs,
flush-before-save-acknowledgement and uncertain-I/O quarantine. Only test memory
adapters implement its `sector_io` trait today. Its 31 printable-byte content
capacity is a proof boundary, not a filesystem limit. Two alternating checkpoint
pairs are not independent media replicas or a backup. Its persisted epoch does
not protect against restoring an older disk, and it has no durable per-request
receipt ledger.

The current Linux Antimatter checkout remains an active Electron/React/CodeMirror
application with Node filesystem services and unrelated uncommitted work. The
read-only audit used `renderer/src/environment/store.ts`,
`renderer/src/environment/editorBridge.ts` and `src/main/environment/files.ts`:
open establishes content and a saved baseline; save retains the submitted
snapshot while later edits can remain dirty; rename preserves the open buffer;
selection replacement collapses the caret after the inserted text. Host
`writeFileSync` and modification-time checks do not themselves implement the
native atomic durable-save contract. Closing an editor is not a durable save.
No sibling files were changed.

The bounded native `editor_state::insert` in
`crates/anomalous-editor-contract/src/lib.rs` rejects a second changed insertion
while `previous` is occupied. Its scripted undo fixture is not a usable repeated
editing loop. Interactive input, selection/delete behavior and bounded undo
history remain a separate integration gate, alongside live disk persistence.

## Selected architecture

Accepted [ADR 0062](../../docs/adr/0062-design-native-object-filesystem.md) records
the requirements. The [filesystem architecture](../../docs/architecture/filesystem.md)
defines stable store/object identity, immutable versions, live and snapshot
views, namespace bindings and fresh task-bound authority as separate concepts.
Rename preserves identity; path reuse cannot retarget an open editor. Native
components preserve exact UTF-8 bytes, with explicit separator/traversal rules,
safe display escaping and incremental validation. Chunked names and scoped
traversal avoid fixed native component, path and depth ceilings while retaining
finite credits, checked arithmetic and complete failure cleanup.

Copy-on-write project roots and exact-version transactions reserve completion
metadata before publication. Checksums and structural validation have distinct
roles. Corrupt latest state must not silently become an apparently healthy older
head. Snapshots preserve history; independently qualified mirrored storage and
backups protect different failure cases. Backup completion needs receiver-side
verification and durable publication, followed by an actual independent restore
test. Restoring bytes does not restore live grants or reset security freshness.
User-authored metadata is durable information; derived semantic indexes remain
rebuildable and cannot confer authority.

Primary Btrfs documentation on copy-on-write, tree checking, scrub, send and
subvolumes informed the requirements. Links and short attributions are in the
architecture's source section. No Btrfs format compatibility or superiority in
performance/reliability is claimed. Identifier encoding, tree format, checksum
selection, encryption and key recovery remain separate design gates.

## Immediate native implementation proposal

Proposed [ADR 0063](../../docs/adr/0063-prove-native-durable-document-storage.md)
specifies the next live proof, without claiming implementation:

1. Qualify a narrowly owned device path to a dedicated disposable data disk,
   separate from the boot image. Legacy ATA PIO is a candidate; q35 and the
   current VirtualBox SATA/AHCI controller do not establish legacy port decode.
2. Enforce exact service/task generation, sealed extent and complete checked
   copies before any I/O. IF-clear polling needs a finite work budget, not a
   PIT deadline. Uncertain commands poison the grant until actually fenced.
3. Let userspace recover and durably advance the mount epoch, then announce
   readiness over an authenticated task-bound route before the editor opens.
   Keep disk parsing and policy outside the kernel.
4. Save independently generated challenge bytes, destroy editor and service,
   and recover under fresh ownership. Repeat in a completely new VM process
   against the retained disk, without supplying expected recovery text to it.
5. Independently compare bytes and outside-extent hashes; test interruption,
   stale authority, lost reply and cleanup. Cold VM restart qualifies the
   selected virtual storage contract, not physical host power-loss behavior.

Existing RAM/native/ELF baselines remain separate. The proposal preserves
VirtualBox's current application CPU-profile rejection until that constraint
has its own qualification; QEMU remains first for the new native proof. The
running interactive development VM is not a disposable test disk or fixture.

## Review and validation

Independent review found no blocking conflict across ADR 0062, the detailed
architecture and ADR 0063. Clarifications distinguish durable publication from
permanent history retention, recovered content from a missing request ledger,
and mount epochs from fresh authority after media rollback. A further pass
separates authored metadata from rebuildable annotations.

This checkpoint changes documentation only. No kernel, service, tooling or
artifact bytes were changed, and no builds, tests or VM processes were started.
A local link check passed all 422 targets across the ten changed/new Markdown
files. Whitespace checks passed. Existing native qualification remains
historical evidence, not a new filesystem result.
During this work the host reported 21,522,382,848 bytes available and zero
`oom_kill` events, with nearly all 8 GiB of swap occupied. No `anomalous-*`
development scope was running. Future implementation retains serialized,
memory-capped jobs and the available-memory/OOM guards from prior qualification.
