# ADR 0118: Add bounded indexed storage

- Status: Accepted — experimental host profile
- Date: 2026-11-21
- Builds on: ADRs 0102 and 0105–0117
- Scope: Experimental host profile 14 for integer array writes and growth and explicitly released integer buffers.

## Context

The complete `aggregates.matrix` edition uses `SET_INDEX`, `CALL array__push 2`,
`ALLOC i64`, and `FREE`. The pinned NIL VM represents literal arrays using
shared heap handles. Its explicit allocations return integer addresses.
The host interpreter needs shared mutation and release semantics with
bounded, execution-owned storage.

## Decision

Select profile 14 explicitly through `indexed_storage::encode` and
`indexed_storage::admit`, with tag 14 in both existing envelope headers.
The opt-in `conformance::validate_indexed_storage` entry point returns the
separate raw validation witness. Earlier profiles keep their existing
envelope tags and grammar. Production admission continues to require
authored main intent, exact edition identity, and an explicit service ceiling.

- `SET_INDEX` consumes an integer array or buffer, an i64 index, and an i64
  value. The instruction mutates the addressed cell and has no result.
- `CALL array__push 2` consumes an integer array and an i64 value and appends
  it. Existing local and declared function resolution take precedence over
  this fallback. The builtin has no result and derives no service authority.
- `ALLOC i64` consumes an explicit integer count and returns an opaque
  integer buffer with zeroed cells. The effective count is at least one.
- `GET_INDEX` reads from live buffers as well as existing array kinds.
  Indexing outside a live integer extent produces zero on read and leaves
  the extent alone on write, matching the pinned default bounds behavior.
- `FREE` consumes a live buffer and clears its cells. Every alias becomes
  invalid. Indexed access or another release through such an alias returns
  `buffer_released`. This is a deliberate policy difference from the pinned
  VM's integer address behavior.

Buffer handles travel through the current function's operand stack and
locals. The buffer kind is distinct from integers and arrays and has no
source signature spelling. `LEN` retains its array contract. Text arrays
retain their existing read-only operations. Arithmetic, casts, console
output, raw pointers, pointer opcodes, and service authority rules retain
their existing operand restrictions.

### Bound storage across the whole execution

Arrays and buffers share 128 descriptor slots and 1,024 scalar backing
cells. Descriptors identify the entire live extent, so copied array handles
observe the same writes and growth through every alias. An append extends
the latest extent in place if it ends at the arena tail. Otherwise, append
moves the array to a fresh extent, charges the full new length, and clears
the previous extent while keeping the same descriptor. Capacity checks
happen before cells or descriptors change. Exhaustion reports
`array_limit`.

Released buffers leave a tombstone. Neither release nor relocation refunds
the cumulative cell budget, and descriptor identities are never recycled
during an execution. The current run owns the complete arena. Every public
completion path clears all storage, including cells, tombstones, and aliases
in frames and operands. The core remains fixed-capacity, allocation-free,
and unsafe-free.

## Evidence

Preserve the complete retained invocation 8 and compare the full signed
result, console bytes, and metadata with the actual pinned VM. Cover
aliasing, writes, array movement, live bounds, allocation, release, and
capacity limits with executed fixtures. Record stale-buffer behavior and
capacity refusals separately from matched VM semantics. Run the earlier
language suites, compare all retained raw and production invocations, and
complete repository and native continuity checks.

All 50 qualification jobs passed on their first attempt. Fifteen language
suites cover 1,532 boundary cases and the reference guard jobs cover 636
checks. The unchanged corpus replay matches 20 of 45 raw invocations and
14 of 38 roots. All 45 production admissions remain refused. The
[Indexed Storage Log](../../notebook/Development/2026-11-21%20Indexed%20Storage%20Log.md)
records the proof, repository checks, and current limits.
