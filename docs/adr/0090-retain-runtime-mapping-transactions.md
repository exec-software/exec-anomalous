# ADR 0090: Retain runtime mapping transactions

- Status: Accepted for the bounded host mechanism; live integration remains open
- Date: 2026-10-13
- Builds on: ADRs 0012, 0041, 0042, 0046 and 0089

## Context

The unchanged interpreter now requests its first libc mapping. The observed
`mmap(NULL, 2231696, PROT_READ, MAP_PRIVATE | MAP_DENYWRITE, 3, 0)` requires
ownership of 545 virtual pages, with 535 populated pages from the retained
2,190,608-byte source. The final populated page contains 3,344 source bytes and
752 zero bytes; ten whole pages remain inaccessible. The
[Linux mmap contract](https://man7.org/linux/man-pages/man2/mmap.2.html) separates
the partial-page zero tail from whole pages beyond EOF, permits descriptor
closure without invalidating a mapping, and ignores `MAP_DENYWRITE`.

The current committed launch owner forbids replacement or modification of its
payload. Its nine-region admission, copy validation and retained root model
cannot acquire another mapping by increasing a page budget. The allocator borrow
also encloses guest execution. Runtime mapping therefore needs an independently
retained transaction before a native service can change a suspended caller.

## Decision

Add a kernel-private owner for an isolated runtime mapping. Its native input
describes a complete virtual extent and a populated prefix with permissions.
The complete extent participates in collision checks against existing mappings
and reservations, including collisions confined to the inaccessible tail.
The prefix alone consumes data frames; the remaining tail is excluded from the
isolated mapping plan. Complete virtual ownership is retained separately from
that exclusion. Construction remains bounded by explicit virtual, data and
table budgets, checked arithmetic, lower canonical addresses and W^X.
Retain the raw requested byte length separately from page rounding. Initialized
bytes may extend through the complete final mapped page even when the raw
request ends earlier; the kernel must not force valid source bytes in that page
to zero. A zero-length populated prefix is unsupported in this first owner.

Reuse `staged_mapping` for real allocator leases, clearing, page-table
construction, independent translation verification and retryable cleanup.
Retain a borrow of the backing owner through the mapping's lifetime. This borrow
does not authenticate a numeric file identity or confer physical access; the
embedding must retain the actual immutable source and independently authenticate
every operation. A descriptor can disappear while the mapping keeps its source.
The population callback must copy and independently verify the intended bytes
and zero padding through independently live privileged access. Its success is
an explicit unsafe contract, not evidence supplied by a userspace boolean.

Preparation errors retain every acquired lease. No failed build or population
callback may publish a partially prepared owner. An unpublished transaction can
be cancelled through explicit cleanup; failed cleanup retains the remaining
leases and may be retried. There is no fallible destructor and no implicit
ownership release.

Publication is a separate unsafe adapter boundary. Before entering its callback,
mark the owner uncertain so unwinding cannot leave it apparently unpublished.
A known rejection certifies that no externally visible mapping remains and
preserves the prepared owner for retry. Confirmed publication retains the owner
as published. An unknown result retains it as uncertain and forbids replay.
Both published and uncertain owners forbid cleanup until the adapter explicitly
establishes complete withdrawal.

Withdrawal must cover every external parent entry, saved continuation, hardware
walker and cached translation that could reach the owned tables or data. The
isolated root being inactive is insufficient: a different root could reference
the same child tables. The adapter must quiesce consumers, withdraw external
references and perform the required invalidation before authorizing cleanup.
This checkpoint supplies the retained lifecycle; it does not implement that
hardware adapter or claim that a callback result proves a physical page-table
change.

## Qualification boundary

Host tests use the real frame allocator with a separate simulated memory
backend. An independent table walk checks every page of the observed libc-shaped
extent, exact initialized bytes, zero padding, permissions and the inaccessible
tail. Smaller fixtures exercise every allocation, table-write, population and
cleanup failure point, plus the first and last readback entry of each table in
both validation passes, without multiplying large host allocations. Tests cover
collision and budget boundaries, known rejection/retry, uncertain and unwinding
publication, blocked cleanup, explicit withdrawal and complete resource return.

Required repository checks and affected mapping/application regressions run
serially under the existing OOM guard. Existing guest images and transcripts
remain regression evidence only. No new live operation, wire format, Linux
syscall result, root splice, TLB behavior or signal delivery is qualified here.

Qualification passed fifteen new transaction tests within the 626-test kernel
suite, strict lint and all 29 required/affected regression targets. The final
audit binds the unchanged source snapshot and five identical development images.
The retained timeout, whole-ELF comparison failure and host-pressure-aborted
workspace attempt remain explicit; the resumed checks passed without relaxing
the guard, and no OOM kill was observed. The
[Runtime Mapping Transactions Log](../../notebook/Development/2026-10-13%20Runtime%20Mapping%20Transactions%20Log.md)
records the evidence, loaded-image comparison limits and remaining integration.

## Integration remaining

End the bootstrap allocator borrow before entering userspace and reacquire it
only under the existing exclusion rules; do not retain a raw allocator pointer
or alias a live borrow. Bind runtime mapping intent to the authenticated pending
caller and retained source. Add combined mapping/circle/copy validation and an
atomic page-table publication path that preserves the caller's original mappings
and frame until reply commit. Retain full virtual ownership beyond descriptor
closure and through uncertain acknowledgements or dependent-task retirement.

Then qualify actual native preparation, publication, withdrawal and failure
windows before answering the observed Linux request. Remapping, protection
changes, executable admission, unmapping and Linux fault delivery retain their
own contracts. Completion of `true` and execution of Antimatter remain open.
