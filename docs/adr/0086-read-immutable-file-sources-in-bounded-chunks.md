# ADR 0086: Read immutable file sources in bounded chunks

- Status: Accepted portable host foundation; native integration pending
- Date: 2026-10-13
- Builds on: ADRs 0084 and 0085

## Context

The portable file service retains a complete immutable byte slice. The native
veil can now read an independently retained libc source through operation 23,
but its 8 KiB writable mapping cannot hold the 2,190,608-byte file. Connecting
these foundations requires a bounded source interface without duplicating the
descriptor table or weakening publication and offset semantics.

## Decision

Generalize the opt-in userspace file service over a statically dispatched
immutable source. Keep byte slices as the default implementation and preserve
the existing slice-backed API. A snapshot retains its exact object, version,
source reference and unsigned 64-bit length. Descriptors borrow that snapshot
for their complete lifetime; replacing a namespace view cannot retarget an
already open descriptor. Equal object/version identities in one namespace must
also have consistent source bindings and lengths.

The source supplies exact reads into caller-owned scratch and accepts a mutable
context for each invocation. The embedding must authenticate that context and
bind the read to the current source grant, task/service incarnations and pending
request. An integer identifier, equality comparison or successful host fixture
does not confer authority. Source implementations must keep bytes immutable and
reject revoked, stale or otherwise unavailable bindings; the generic service
cannot independently authenticate a provider's content.

Compute the requested extent from the retained length, descriptor offset,
requested count and explicit byte budget. Scratch must hold that entire extent;
an undersized buffer is rejected rather than silently changing the budget. A shorter provider
reply is an error, including at a claimed EOF: the service has already bounded
the request to the snapshot. Overreported lengths and source errors are also
errors. No publication callback runs for these failures, even if the provider
dirtied a scratch prefix. Descriptor offset and committed sequence stay intact.
Zero-byte requests and EOF publish an empty read without calling the provider.

Read payloads borrow scratch only during the synchronous publication callback.
Commit descriptor state only after confirmed publication. Known rejection
permits retry without advancing the offset; unknown publication or publication
callback unwinding fences the service and revokes its descriptors. This retains
ADR 0084's embedding obligations and does not make external writes transactional.

Clear the entire supplied scratch buffer on success, rejection, validation or
source failure and Rust unwinding. This is logical clearing in safe Rust; it is
not a compiler-resistant physical erasure guarantee, and aborting faults cannot
run destructors. Native deployment must separately qualify required volatile
clearing, fault cleanup and any copies retained by its transport.

The abstraction introduces no global file-size, pathname-length or depth limit.
Descriptor capacities, scratch size and per-invocation work budgets remain
explicit deployment choices. It adds no kernel operation or stable wire format
and remains absent from the default native/Linux profiles.

## Qualification and limits

Exercise both existing byte slices and the new source interface through the
same descriptor state machine. Test wide extents, arithmetic boundaries,
independent offsets, retained versions, source retirement, stale contexts,
partial dirty failures, short/overreported replies, zero/EOF reads, publication
rejection, uncertainty and unwinding. Check scratch after every returning path.

The host qualification must reconstruct the complete pinned libc through two
independent descriptor tables and bounded source reads. Retain the actual byte
streams and operation traces for an independent verifier. Fixture authority and
injected source failures qualify the portable contract only.

Preserve the existing three-return guest proof and run required regressions
under the unchanged serialized memory guard. Exact native artifact identity
permits carrying forward its emitted review; any changed native instructions
would require a new review before execution.

Native integration remains the next checkpoint: authenticated caller/service
incarnations, operation 23 as a concrete source provider, operation 19 staged
copyout and operation 20 completion. The callback may remain suspended inside
operation 20 until the next Linux boundary; it must reconcile the previous
publication before processing that new request. Veil faults or uncertain
acknowledgements must contain the dependent caller. This checkpoint establishes
none of those native guarantees, another Linux return, a successful guest file
open/read, libc startup, `true` completion or Antimatter execution.

## Qualified checkpoint

Both bounded-source clients reconstructed all 2,190,608 pinned libc bytes.
Independent verification checked 2,755 source operations and 2,744 backend
calls, alongside the existing 2,739-publication slice proof. All 63 all-feature
protocol tests, 362 host-tool tests, strict host/freestanding lint and all 29
required/affected commands passed on 486 unchanged frozen source files.

Fresh TCG/KVM runs at 96/320 MiB preserve the exact earlier native artifacts and
three-return loader behavior. All five development images remain byte-identical.
The final audit covers all six guarded attempts and 49 jobs; every attempt
passed. Peak scope memory was 2.57 GiB, with no observed OOM or pressure abort.
VirtualBox was not rerun for this portable-only change; its earlier capability
refusal still applies to the unchanged native image. Exact receipts and the
native integration handoff are recorded in the
[Bounded File Backend Log](../../notebook/Development/2026-10-13%20Bounded%20File%20Backend%20Log.md).
