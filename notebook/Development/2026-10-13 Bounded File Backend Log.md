---
title: "Bounded File Backend Log"
date: 2026-10-13
status: qualified-portable-foundation
---

# Bounded File Backend Log

The starting revision is `06ff88ff3f91973d673cb066e59bff4f0a91089b`.
The next foundation under [ADR 0086](../../docs/adr/0086-read-immutable-file-sources-in-bounded-chunks.md)
joins the portable file service's descriptor/offset model to a bounded immutable
source interface. The existing Linux Antimatter/Electron build remains the
application target. This checkpoint does not add a Linux guest reply.

## Implementation observations

The generic service uses static dispatch and keeps the existing slice-backed
API as its default. Source lengths and offsets remain `u64`; only each bounded
transfer becomes `usize`. Scratch capacity is checked independently of the
requested work budget. The borrowed source and retained snapshot length survive
namespace replacement through every open descriptor.

Before and after source access the service checks the retained length. Exact
provider delivery is required before invoking publication. Dirty errors and
wrong returned counts expose no payload and preserve offset and freshness.
The provider still owns the immutable-content and authenticated-context
contract; those facts cannot be inferred from a trait, identifier or hash.

Scratch clearing covers the full supplied buffer, including unused tail bytes,
on all returning paths and Rust unwinding. It is logical safe-Rust clearing,
not a volatile physical-erasure guarantee. Source or publication unwinding
fences the service. Source errors with a known failed outcome are retryable
without advancing committed state. Publication uncertainty revokes descriptors.

## Native integration handoff

The existing native operation 20 can suspend the publication callback until
the next real Linux boundary. On resumption, the adapter must validate the
acknowledgement and commit the previous local effect before fetching the new
request. There is no need to create a separate asynchronous portable descriptor
state machine merely because the scheduler suspends that callback.

Reuse the existing operation 19 copyout stage and its complete destination
preflight, then revalidate under prepared reply completion. Supply caller and
service incarnations through trusted deployment/controller state; the copied
ANVR request arguments do not authenticate them. Contain the veil and dependent
caller on uncertain publication or faults rather than replaying accepted work.

The current native RW mapping is 8 KiB and already uses 4,304 bytes. Reuse the
operation 23 source buffer through staging and completion; a second 4 KiB
buffer would exceed that mapping. The reviewed bootstrap stack uses 46,480 of
65,536 bytes, leaving only 2,672 beyond its required 16 KiB reserve. Keep any
expanded journals/stages out of growing by-value bootstrap observations and
perform fresh emitted review for a native integration change.

Successful guest file operations, mapping/protection, dependency startup,
TLS/libc startup and a real `true` exit remain open before the larger Electron
environment. No native Antimatter frontend rewrite is needed.

## Qualification evidence

Task-local evidence is retained under
an external validation directory (not distributed).
The previous `linux-file-source` and `linux-file-service` evidence is unchanged.
The guard remains SHA-256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`:
one build worker and test thread; 4 GiB memory high, 6 GiB maximum, 512 MiB scope
swap maximum; 30 seconds of quiet pressure before jobs, 4 GiB available floor,
and unchanged host/ancestor/scope PSI abort thresholds.

The focused run passed 63 all-feature protocol tests, 362 host-tool tests and
strict host/freestanding lint. Nine new file-service tests bring that module to
25 tests. The initial core-only compile/test attempt also passed; it remains
supplemental evidence before the final source freeze.

The existing command now retains both its original slice proof and an additional
bounded-source proof. All four delivered streams equal the pinned 2,190,608-byte
libc, SHA-256
`85e64f97e348786a8fb4d9f3d52fec289e2fb86bba20f0731dfe61990525e0f7`.
The independent Python verifier replays 2,739 legacy read publications and
2,755 bounded-source operations, including 2,744 provider calls. It checks
actual callback/backend bytes, offsets, seven source failures, same-sequence
publication retry, zero/EOF omission, retained versions and unknown fencing.
Its receipt `host-proof-review-v1.json` has SHA-256
`5396a20bea15158cc3d4c95345d2346e300bf71d952fd1d7ec6756c8afffd243`.

Sixteen native artifacts match the prior reviewed checkpoint byte-for-byte,
including the kernel, native veil, loader image, process source/plan and file
source/binding. This carries forward the existing kernel/native emitted and RW
mapping reviews, without claiming deployment of this new portable backend.
Fresh TCG and KVM runs at 96/320 MiB each pass: four raw calls, three returns,
666 native calls, 536 successful source reads and ten rejected source reads,
with complete retirement/restoration. The source stays separate and the fourth
Linux call remains unanswered. No boot code changed; VirtualBox was not rerun.
Its prior `application_cpu_amd_error_pointers` capability refusal remains the
known limitation for this exact image.

All 29 required/affected commands passed, followed by restoration of the usual
owned-CPU build. All five development images match the starting hashes. The
final qualification scopes contain 48 jobs, including formatting and the source
freeze; the earlier core-only test is retained separately. These total 49 jobs
across six guarded attempts, all successful.
All 486 non-Markdown source hashes remain unchanged from
`source-freeze-v1.json`, SHA-256
`37c3dc1ee8835a4b43e6c26e812f6fc0fd8ee54eb5cb23e8d0c3fabc1675296a`.
Documentation is finalized separately after qualification.

Peak scope memory was 2,759,553,024 bytes (2.57 GiB); minimum observed host
available memory was 16,779,509,760 bytes (15.63 GiB). No pressure abort,
host/scope OOM counter increase or matching accessible OOM/oomd journal event
was observed. Collected transient units and brief intersample events are not
claimed as complete historical telemetry.

Independent evidence review found three audit gaps before final acceptance:
native identity needed a direct link to the pinned emitted-review hashes,
passing receipts needed their bound file hashes rechecked, and scope inventory
needed to detect interrupted attempts without summaries. All three were
corrected and independently re-reviewed. The original identity scripts and
receipts remain retained; supplemental `*-identity-v2.json` receipts establish
the stronger review linkage without rerunning unchanged guest code.

The final manifest is `final-manifest-v1.json`, SHA-256
`c2787ce81223f3ce03723c3d13acdcdf73002b882a495c5fa13f36f5b53e9380`.
The successful `final-audit-v1/receipt.json` has SHA-256
`d06d265c0d778ad7eae475d9be75c89062efbac4b5c05922605e05afa3d9a0d3`.
It binds source, tests, guard attempts, independent host verification, native
review/artifact identities, physical transcripts and unchanged baselines.
