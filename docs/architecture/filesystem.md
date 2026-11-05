# Native filesystem direction

## Status and boundary

This is architectural direction, not an implemented filesystem or a stable
on-disk or wire format. It selects requirements for a native filesystem service
without selecting a tree layout, identifier encoding, checksum algorithm, or
device protocol. Those choices require separate implementation and qualification.
The direction is recorded in
[ADR 0062](../adr/0062-design-native-object-filesystem.md).

The current [durable document model](../adr/0060-model-bounded-durable-document-recovery.md)
operates over five explicitly granted sectors and one bounded document. Its
portable recovery proof is separate from the live native adapter added by
[ADR 0063](../adr/0063-prove-native-durable-document-storage.md). That opt-in
profile now proves normal save, full service destruction and cold QEMU recovery
from a dedicated disk. Neither proof is a general filesystem. The small record
and identifier fields are fixture limits, not the future native naming or
object model. Native failure cuts and lost-reply cases remain open.

Filesystem parsing, namespace policy, transactions, recovery, and indexing
belong in supervised userspace services. The kernel supplies bounded task,
memory, capability, IPC, and device mechanisms. It does not interpret paths,
document formats, project names, or AI annotations. The same semantic rites
serve Antimatter, Canticle, accessibility clients, and agents.

## Identity, versions, views, and authority

These references have different meanings and must remain separate:

| Reference | Meaning | Does not establish |
| --- | --- | --- |
| `(store, object)` | Stable logical identity within a store | A name, current bytes, liveness, or permission |
| Immutable version | One committed state of that object | The current state of every branch |
| Branch or snapshot view | A selected project root and its object/version bindings | Authority to read everything reachable |
| Task-bound handle | Live, bounded authority over an object/view or operation | Permanent authority after close, revocation, or restart |

Object identities are opaque, never recycled for another logical object.
Physical extent reuse does not reuse identity. Content hashes can identify
or validate bytes, but equal bytes need not mean equal objects. Neither a
guessed identifier nor a known hash grants access. A view is explicit when
different branches can bind the same object to different versions.

Renaming changes namespace bindings while preserving object identity. Saving
checks the expected base version in the selected live branch and publishes a
fresh immutable version of the same object. Replacing a path with a distinct
object is a separate rite. An already opened handle never silently retargets
when a name is removed and reused.

Unlinking a name, deleting an object from a live view, revoking a grant, and
reclaiming its extents are separate effects. A read handle may pin a version
under its existing grant; a snapshot may retain historical bytes. Neither pin
creates new permission, and revocation still applies. Reclamation requires
all retained roots and pins to release their references.

## Names without global length or depth limits

Native components are nonempty, valid UTF-8 byte strings. They are case-sensitive
and preserve the submitted bytes exactly. NUL, slash, and the reserved components
`.` and `..` are excluded from stored names. No normalization, case folding,
truncation, or character substitution silently changes identity. A textual path
frontend may interpret traversal syntax before submitting scoped operations.

[ADR 0061](../adr/0061-select-re-extension-and-path-policy.md) excludes fixed
global native path length and depth ceilings. The same requirement applies to
individual native names: no inherited `PATH_MAX`, fixed name array, or fixed
small length field becomes the filesystem's semantic limit.

Directory entries refer to chunked name blobs rather than requiring a complete
name to fit one metadata block. Requests stream components and traverse through
scoped directory handles, avoiding allocation of one contiguous absolute path
or recursion proportional to depth. UTF-8 validation retains state across chunk
boundaries; an incomplete or invalid final sequence cannot publish a binding.
Hashes can accelerate lookup, but a collision requires exact byte comparison.

Transport chunks, buffers, counters, allocations, traversal work, and retained
cursors have explicit finite credits charged to the requesting authority.
Checked arithmetic and resumable work expose exhaustion and cancellation as
structured results. Representation exhaustion is an explicit failure, not
wrapping or aliasing. A failed request publishes no partial namespace change.
Increasing a proof budget must not require changing native name semantics.

Case-folded or normalized lookup is an explicitly selected, versioned search
view that can report several exact names. It cannot silently select one match
for mutation. Foreign adapters report their own naming limits and collisions
without imposing them on the native store.

Names and metadata are untrusted data, never executable commands or prompts.
Interfaces escape controls and make bidirectional formatting visible while
preserving the underlying bytes. Display shortening must remain distinguishable
from the complete name and cannot supply the mutation target.

## Namespace and traversal

The authoritative namespace starts as a directory tree within one store.
Directories have one parent per view; directory hard links and symlinks are
deferred. Renaming a directory beneath its descendant is rejected. Cross-store
links, mounts, and federated traversal require later contracts.

A named file binding is a semantic reference to an object. Adding another
binding requires the relevant directory and target authority; knowing a target
identity is insufficient. Opening through a binding rechecks access policy.
Permission to enumerate a directory does not automatically authorize reading,
executing, changing, or delegating its children.

Lookup begins from an authorized directory/view handle. Every step applies
disclosure policy before returning names or metadata. Resolution cannot become
an unrestricted global search preceding authorization. Native object references
let clients reopen authorized targets without retaining long historical paths.

Rename and replacement atomically check the expected source/destination directory
versions and bindings. A concurrent replacement yields a conflict instead of
acting on whichever object later occupies the name. Initially, one writer
serializes publication; optimistic preconditions still matter for stale clients.

Enumeration pins a committed view and returns a bounded continuation tied to
that view and caller. Resume checks current authority. A changed live namespace
does not mix generations into that enumeration, and revoked access stops further
disclosure. Pins consume credits and have explicit release and expiry behavior.

## Copy-on-write publication and exact saves

Data and metadata are copy-on-write and checksummed. Structural validation
checks lengths, ordering, references, ownership, and extent bounds independently
of checksum validity. Checksums detect corruption under a declared model; they
are neither authorization nor authentication against hostile modification.

A transaction stages new extents and metadata without overwriting the currently
published state. It builds an immutable project root covering the participating
namespace and object versions. Publication atomically selects that root through
a recoverable commit protocol; this is a logical guarantee, not an assumption
that an arbitrary disk write is physically atomic.

Before committing, reserve the data and completion metadata needed to publish
or abandon the transaction, including allocation and reclamation bookkeeping.
Insufficient space rejects before namespace publication. Cancellation before
publication abandons staged state; after possible publication, report a committed
or unknown outcome instead of pretending cancellation rolled the write back.
Orphan cleanup must not reclaim extents reachable from any retained root.

A save carries an operation identity, object/view, expected base version, and
an exact snapshot of bytes. Commit rechecks authority and compare-and-swap
preconditions. A durable acknowledgement binds that request, accepted bytes,
and resulting version after the selected durability profile's flush requirements
are satisfied. Newer unsaved editor changes remain dirty after an older save's
acknowledgement. A conflict changes neither committed content nor saved baseline.

The future filesystem needs a durable, bounded request/outcome ledger committed
consistently with the root to reconcile retries after a lost reply. Its retention,
expiry, and retry identity must be explicit; an expired result cannot imply the
write never happened. Reusing a request identity with different bytes or
preconditions is rejected, and outcome queries require live authority.
This ledger is a requirement, not an implemented property
of the current `durable.rs` model. Uncertain writes or flushes require quarantine
and device-command fencing before reuse; clients keep the save outcome unknown.

## Durability profiles and recovery

The first profile is local, single-device durability under an explicit adapter
flush contract. It does not survive loss of that device by definition. Emulator
cold boots can qualify the selected virtual device contract; they cannot prove
physical power-loss behavior or that underlying hardware truthfully flushes.

Initial recovery is read-only and never formats, patches metadata in place, or
recomputes checksums to make damaged bytes acceptable. Validating a root includes
its required reachable structure, not just its top-level checksum. Validation
can be incremental within accounted budgets; incomplete validation is not success.

Recovery after a covered interrupted publication must select a complete old or
new root and must not roll back a durably acknowledged publication under the
stated failure model. Historical versions remain subject to explicit retention;
a later acknowledged commit may supersede earlier contents.
If corruption makes the latest root uncertain, an older readable root is an
explicit degraded recovery view. It must not silently become a healthy writable
head or hide possible loss of acknowledged work. Scrubbing, corruption repair,
and recovery from hostile rollback need separate evidence beyond power-cut tests.

A future `mirrored2` profile requires two replicas in independently declared
failure domains and durable acknowledgement from both before reporting success.
Two VM disk files on one physical disk do not demonstrate independent-device
survival. Missing members, stale replicas, or conflicting heads do not cause
automatic degraded writes or a choice based only on the largest sequence number.

Membership changes and writer epochs require fencing that prevents two accepted
writers after a partition or restart. Explicitly entering a weaker durability
profile changes the disclosed contract; it cannot preserve the old promise.
Initial work excludes distributed consensus, parity, deduplication, compression,
and automatic repair. None is required to prove the first editor save.

## Snapshots, backups, and restoration

A snapshot pins a complete immutable project root with its committed names and
object versions. It excludes unsaved application buffers. Multi-project or
application-consistent snapshots require separately defined coordination.
Reverting a document publishes historical bytes as a new live version; it does
not rewind version preconditions or a security ledger.

Snapshots provide history; replicas provide the declared availability or device
failure tolerance; backups provide independently retained recovery material.
Replication can propagate deletion or corruption, and snapshots can share the
same damaged extents. A backup needs a distinct retention policy and failure
domain, with deletion authority that does not automatically follow the source.

A full backup is self-contained for its declared scope. Its versioned manifest
covers content, exact names, object relationships, versions, provenance, required
metadata, and hashes. External exclusions are explicit. An incremental backup
names exact immutable bases and includes their complete dependency closure or
verifies that the receiver already retains it. Missing bases prevent completion.

The sender pins the source root. The receiver validates framing, references,
bounds, hashes, and the reconstructed root before durable publication. Completion
requires a receiver receipt for that published root, not merely an exhausted
stream or successful network send. Interrupted imports remain incomplete and
cannot replace a valid backup. Restore testing verifies bytes and names from
independent media; hash integrity alone does not establish trusted provenance.

Restoration establishes fresh task-bound authority from a source that the restored
backup cannot rewind. A disk mount counter or restored policy database alone
does not prove freshness after rollback. Without that proof, deny activation.
Archived ownership and policy metadata are reviewed recovery inputs, not live
credentials; old handles, grants, request authority, and sessions never replay.

Cloning creates a new store identity and an explicit provenance map to source
objects/versions. Restoring the original logical store requires fencing any
surviving original writer as well as fresh authority. Neither operation silently
merges independently writable histories. Exact identity allocation, authenticated
backup transport, encryption, key recovery, and key-loss behavior remain future
gates; snapshots and identity recovery do not solve cryptographic recovery.

## Search, AI context, and semantic operations

Search indexes, inferred tags, embeddings, summaries, and generated annotations
are derived, version-bound, rebuildable state. They cannot become the only copy
of a name, document, or authority decision. Their provenance records which
object version they describe. Stale results are labeled or rejected, not
silently current. User-authored tags and notes are explicit versioned metadata
included in transactions and backups; deleting a derived index cannot erase
authored information.

Queries filter names, content, metadata, and result counts through the caller's
authority. An index hit or model-generated object reference grants no access.
Stored annotations and retrieved text remain untrusted data, including apparent
instructions embedded in filenames or document content.

A preview freezes the selected target identities, views, versions, effects,
costs, and required grants. Commit revalidates those exact targets and authority;
it does not rerun a changing search and mutate new matches. Conflicts require a
new preview. People and agents use the same discover/preview/commit/result rites,
including attributable progress, cancellation limits, and recovery outcomes.
See [Canticle](canticle.md) and [identity and authority](identity-and-authority.md).

## Failure obligations

| Failure or race | Required result |
| --- | --- |
| Rename or path reuse after open | Existing handle remains on its original object; stale binding preconditions fail. |
| Save from an old base or view | Conflict without changing the live root or editor baseline. |
| UTF-8 split, malformed final name, or hash collision | Incremental validation and exact comparison; no partial binding or alias. |
| Deep traversal, credit exhaustion, cancellation | Bounded resumable work or structured failure with complete resource cleanup. |
| Namespace cycle or unauthorized link target | Reject before publication; no new reachability or authority. |
| Space exhaustion while staging | Preserve old namespace; completion metadata reservation prevents a half-published transaction. |
| Torn write, failed flush, or lost acknowledgement | Known committed result or explicit unknown outcome; fence uncertain I/O before reuse. |
| Corrupt newest root or invalid reachable metadata | Explicit degraded read-only recovery, never a silent healthy fallback. |
| Stale mirror, missing member, or competing writer | Reject the old durability promise; require membership and writer fencing. |
| Interrupted backup or absent incremental base | No complete receipt or activation; retain previously valid backups. |
| Restored snapshot contains old grants or epochs | Fresh external authority proof required; otherwise deny activation. |
| Stale index, malicious annotation, or revoked cursor | No authority from derived data; revalidate targets and stop unauthorized disclosure. |

## Implementation gates

1. **Actual native disk persistence.** Connect the existing bounded document model
   to a narrowly authorized disposable data extent. Prove exact save/read bytes,
   write-free semantic rejection, uncertain-I/O handling, service destruction,
   and recovery after a cold VM boot with the disk preserved. Keep the boot disk
   separate and record flush settings. Proposed
   [ADR 0063](../adr/0063-prove-native-durable-document-storage.md) specifies this
   gate. Its initial normal-path save, pair destruction and cold QEMU recovery
   are qualified; native uncertain-I/O fault cuts and lost-reply reconciliation
   remain pending. It remains ahead of a general filesystem.
2. **Native objects and usable editing.** Introduce chunked document storage and
   names, scoped traversal, exact versioned saves, and an interactive native
   Antimatter path. Prove rename/open identity, long components, deep traversal,
   exhaustion, cancellation, and cleanup without turning proof budgets into
   global limits. Compare the qualified native behavior through ELF separately.
3. **Snapshots and independent backups.** Qualify complete roots, retention,
   space accounting, full/incremental export, receiver completion, and restore
   into a fresh authority instance, including missing-base and corruption cases.
4. **Mirrored durability.** Qualify independent failure domains, acknowledgement
   from both members, recovery, and fencing before offering `mirrored2`.

These gates support native Antimatter running and rendering, then editing and
persisting work. They do not make a general filesystem or distributed storage
system a prerequisite for the first application proof.

## Source principles

These sources inform requirements; the choices above are Anomalous's design,
not an adoption of Btrfs's format, algorithms, compatibility, or guarantees.

- [Btrfs introduction](https://btrfs.readthedocs.io/en/latest/Introduction.html)
  describes copy-on-write storage and data/metadata checksums. Those motivate
  explicit immutable publication and integrity requirements here.
- [Tree checker](https://btrfs.readthedocs.io/en/latest/Tree-checker.html)
  distinguishes structural validation from checksum verification. A valid
  checksum alone cannot establish a valid filesystem structure.
- [Scrub](https://btrfs.readthedocs.io/en/latest/btrfs-scrub.html) distinguishes
  checking from repair using a good replica. Checksums cannot reconstruct
  missing bytes, and scrubbing is not a complete consistency checker.
- [Send](https://btrfs.readthedocs.io/en/latest/btrfs-send.html) documents full
  and incremental transfer with immutable, matching bases. This motivates
  explicit backup dependency closure and receiver verification.
- [Subvolumes](https://btrfs.readthedocs.io/en/latest/Subvolumes.html) explains
  shared snapshot extents and snapshot boundaries. This motivates separating
  local history from independently retained backups and declaring backup scope.
