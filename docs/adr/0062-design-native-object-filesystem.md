# ADR 0062: Design a native object filesystem

- Status: Accepted
- Date: 2026-10-04
- Scope: Architectural direction only; no filesystem implementation, stable disk format, wire encoding or new privilege boundary is accepted here.

## Context

The user requested storage designed for both people and AI: descriptive names
without inherited path limits, stable references, redundancy and easy backups.
Native Antimatter needs real document persistence before this broader system.
The current guest document service is RAM-only. ADR 0060 qualifies a separate
five-sector, single-document recovery model; neither that model nor Threshold's
boot-time reads constitute a filesystem or runtime block driver.

[ADR 0061](0061-select-re-extension-and-path-policy.md) selects `.re` for
Reliquary and rejects fixed native path-length/depth ceilings. This decision
extends that rule explicitly to individual name components. Finite operation
budgets remain essential; avoiding a fixed path limit does not promise infinite
storage, memory or work.

## Decision

### Stable objects with human-readable names

Build the native filesystem service around separate concepts:

| Concept | Meaning |
| --- | --- |
| Store and object identity | Persistent identity independent of location, label and content; not permission |
| Immutable version | One committed state of an object |
| View | A live branch or pinned snapshot used for consistent resolution |
| Directory entry | A name bound to an object within a view |
| Live handle and grant | Fresh task/session-bound authority for exact operations |

Renaming a file preserves its object identity. Editing advances its version.
Replacing a directory entry with a different object never redirects an already
open editor to the replacement. Unlinking, revoking access and reclaiming bytes
are distinct operations. Reachability from snapshots, open handles and retained
versions constrains reclamation; none of those references bypasses authorization.

Native names preserve exact UTF-8 bytes, are case-sensitive and exclude NUL,
the path separator and reserved traversal entries. Normalization, case folding,
search aliases and safe display escaping are explicit projections rather than
silent identity changes. Controls and bidirectional text must be rendered
unambiguously without altering the name used in an operation. Compatibility
adapters report their own representability constraints without imposing them on
native objects.

Use chunked names/content and incremental traversal through scoped directory
handles. There is no fixed native component, path or depth ceiling. Work and
memory credits, checked arithmetic, cancellation and structured exhaustion apply
at every step. Hash indexes must resolve collisions with complete name equality;
truncation, display elision and hashes cannot select a mutation target. The first
authoritative namespace is a directory tree; directory hard links and symbolic
links require later semantics and evidence.

### Transactional content and recovery

The service stages immutable content and metadata, validates checksums and
structure, then publishes one copy-on-write project root. A transaction states
its expected object/directory versions and rechecks authority at commit. A
conflict preserves the existing state and staged work is reclaimable. Atomic
rename with no replacement is an explicit operation; cross-store moves are
separately reported copy-and-unlink workflows.

Reserve the space needed to finish metadata and recovery records before
publication. No-space failure must not partially publish a project or make its
last committed root unrecoverable. Reclamation must respect every live root and
reference; implementation requires independent crash and allocation evidence.

A save receipt identifies the exact submitted snapshot, committed version and
qualified durability profile. It follows all required persistence barriers.
Losing the reply creates an unknown outcome, not proof of failure. The eventual
transaction protocol needs durable request-result records with explicit
retention and outcome queries. ADR 0060 does not implement that ledger: reopening
its store reconciles current content, not exact historical request identity.
An older save completing never marks newer unsaved editor changes clean.

Checksums detect covered corruption; structural validation detects additional
invalid states. Neither supplies authority or guarantees authenticity. Recovery
must distinguish a known committed root from an uncertain latest state. A valid
older root is an explicit recovery option, never silent proof that an
acknowledged newer save survived. Ambiguous or corrupt state is read-only until
an authorized recovery action is selected. Do not overwrite suspect evidence.

### Snapshots, redundancy and backups are separate contracts

Start with one writer and a declared single-device durability profile. Immutable
snapshots pin a committed project root, including namespace and saved content;
they do not include unsaved editor buffers. Restoring historical content normally
creates a new live commit rather than rewinding authority or pretending later
history never occurred.

A later mirrored profile requires two independently qualified failure domains,
checksummed replicas and durable completion on both before a mirrored receipt.
Two files or checkpoint slots on the same physical device do not meet that
profile. Scrub repairs only from a verified good copy; disagreement or uncertain
membership must not select a writer by guesswork. Writer fencing, replacement,
resynchronization and explicit degraded-mode policy are prerequisite proofs.
Do not silently downgrade a mirrored save to single-device durability.

Backups export a pinned immutable root with its complete content/metadata
closure, versioned manifest and provenance. Full exports are independently
restorable. Incremental exports name and verify their exact required bases.
Completion requires receiver verification and durable publication of a receipt,
not just a sender finishing its stream. Retention may remove a base only when
retained restore points remain complete. Qualification must actually restore
with the primary unavailable and compare bytes, names and object relationships.

Restored metadata never restores live grants, task handles or replay authority.
Activation requires a fresh authority incarnation from a qualified source not
rolled back with the backup; a persisted mount counter alone cannot supply it.
Until that source is qualified, restored content remains inactive for mutation.
Clones receive a new store identity and an explicit origin mapping. Portable
policy metadata is intent to reauthorize, not an imported administrative bypass.
Encryption, key recovery and authenticated backup trust require separate
decisions and are not claimed by checksums or this architecture.

### One governed service for people and machines

The filesystem and device drivers remain supervised userspace services under
[ADR 0004](0004-capability-oriented-hybrid-kernel.md). The kernel enforces owned
memory, channels, grants and device mechanisms, not names or filesystem policy.
ADR 0040's distinction between identity and authority continues to apply.

Human UI, Canticle and agent requests share versioned operations, previews,
commit checks, receipts and recovery choices. Bulk edits freeze their target
set and expected versions before commit; rerunning a search cannot silently
change an approved operation. Semantic indexes, inferred tags and AI summaries
are derived, permission-filtered, rebuildable data. User-authored metadata is
versioned and backed up with the objects it describes. Names and annotations cannot
execute commands, enlarge grants or become trusted instructions.

Reliquary `.re` files are governed artifacts stored by the filesystem, not its
disk format. Covenant may pin immutable artifact generations; package rollback
must not implicitly roll back mutable user documents or current authority.

## Delivery and evidence

The [filesystem architecture](../architecture/filesystem.md) records the detailed
contracts, failure matrix and implementation gates. First implement and qualify
the narrower [native durable-document proposal](0063-prove-native-durable-document-storage.md):
save a run-specific document through a scoped device path, destroy both editor
and storage service, and recover exact bytes after a cold VM process restart.
That disposable proof retains its bounded format and is not the first release
of a general filesystem.

Follow with multiple chunked objects, incremental names/directories and a usable
interactive editor; then transactional project roots/snapshots, independently
restorable backup streams, and verified mirror repair. Format encodings, device
boundaries and ABI contracts each need their own reviewed implementation and
failure evidence. No proof capacity becomes a permanent native naming limit.

## Alternatives and consequences

- Path strings as identity would make renames and stale agent references unsafe;
  keep names useful while targeting stable objects and explicit versions.
- Adopting an existing filesystem as a backing implementation remains possible
  behind an adapter. Its name limits and authority semantics must not silently
  define the native object contract. No Btrfs on-disk compatibility is selected.
- Inventing distributed consensus, parity RAID, global deduplication or a general
  query engine in the first disk proof would delay an independently testable
  persistence baseline. Those features are deferred, not implied guarantees.

Copy-on-write, checksums, snapshots and replication are established techniques,
not novelty claims. The intended improvement is measurable native behavior:
references survive renames, large names use bounded memory, operations identify
their versions/effects, and a backup proves that it can restore. This design
claims no superiority in speed, reliability or recovery over Btrfs without
comparative evidence.
