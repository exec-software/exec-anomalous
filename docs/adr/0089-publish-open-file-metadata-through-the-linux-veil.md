# ADR 0089: Publish open-file metadata through the Linux veil

- Status: Accepted for the bounded open-file metadata integration
- Date: 2026-10-13
- Builds on: ADRs 0084, 0087 and 0088

## Context

The unchanged Linux interpreter opens the retained libc, reads its ELF header
and program headers, then requests `fstat(fd=3, statbuf=0x7fff0000ef80)`.
The [Linux stat manual](https://man7.org/linux/man-pages/man2/stat.2.html)
defines descriptor-based metadata lookup with result zero on success. Its
[x86-64 UAPI layout](https://raw.githubusercontent.com/torvalds/linux/master/arch/x86/include/uapi/asm/stat.h)
requires a 144-byte record, including padding and reserved fields.

## Decision

Add transactional metadata publication to the portable immutable-file service.
Authenticate the owner, fresh sequence and exact retained descriptor handle,
then publish the snapshot's object, version and unsigned byte length. This
operation neither reads the provider nor changes descriptor offset, generation
or snapshot binding. Only confirmed publication advances the watermark. Known
rejection preserves state; uncertainty or callback unwinding fences the service
and revokes its handles through the existing publication guard.

The native Linux adapter encodes the foreign record explicitly in its existing
private scratch buffer. This proof's complete library namespace assigns logical
volume 1, object 1 and version 1 independently of the source grant. It presents
one name, a regular file with mode 0444, synthetic uid/gid zero, no special-device
identity, a 4096-byte preferred I/O size, and nonsparse logical block accounting
rounded up in 512-byte units. Timestamps, padding and reserved fields are zero.
These are explicit immutable userspace policy values, not imported host metadata,
credentials, native authority or claims about physical storage allocation.
Linux stat has no version field; the service retains the version binding.

Preserve unsigned lengths in the portable API. The Linux encoder checks signed
size representability before staging; that foreign-format restriction is not a
global filesystem size limit. General errno adaptation remains outside this
bounded proof. The existing stage and completion operations carry 144 bytes
while returning Linux result zero. The delayed native acknowledgement reports
the copied length separately. Wipe private scratch only after the callback's
borrow ends; fault cleanup owns its physical erasure.

Generalize the removable kernel proof's delivery journals by sealed request
sequence rather than source-read count. Source-backed and service-produced
copyouts share admission. Select a fresh pristine journal only after the prior
request committed and cleared its stage; admit capacity before source I/O or
diagnostic copying. Capture the complete first caller witness in scratch before
committing a new journal. Reject inaccessible first destinations without changing
earlier observations. Rejected completions must not increment an earlier
request's counters. An already-staged request cannot stage again.

Retain four journals in kernel BSS, never by-value on a privilege stack. The
kernel observes zero source reads for service-produced data without interpreting
Linux metadata or descriptors. The journal count and 4 KiB transfer budget are
local proof budgets, not filesystem or pathname ceilings.

## Qualification boundary

Run seven fresh lifetimes: normal read/pread/metadata publication, the four
existing sequential/positional-read fault cases, and metadata faults after staging and after accepted
copyout with an injected unknown local result. Preserve prior journals and the
sequential offset of 832. Unknown local outcome follows an exact valid physical
acknowledgement and must fence without replay. Each lifetime must retire the
dependent pair, complete the independent peer and restore owned resources.

The normal lifetime includes the real interpreter's repeated positional read
after metadata. Every nonempty delivery runs its own rejection probes; class-wide
probe bookkeeping cannot suppress checks on a later delivery.

Before the first metadata journal exists, reject a bad destination and a
premature completion while retaining the prior read observations. Each admitted
delivery retains its eight rejected stage probes and four rejected completions.
The independent host oracle decodes every actual metadata field and padding byte,
checks source-free accounting, and distinguishes result zero from copy length.
It requires successful metadata publication before a genuinely unsupported next
request; the real interpreter determines that request.

Fresh emitted control/stack and instruction-semantics reviews precede physical
execution. Required and affected checks remain serial under the unchanged memory
guard. This increment does not qualify libc mapping/startup, completion of `true`,
general Linux metadata operations or Antimatter execution.

## Accepted evidence

Identical reviewed image bytes pass TCG and KVM at 96/320 MiB across seven
fresh lifetimes. The unchanged interpreter accepts the 144-byte metadata record
with result zero, repeats its positional program-header read, then requests
`mmap(NULL, 2231696, PROT_READ, MAP_PRIVATE | MAP_DENYWRITE, 3, 0)` at sequence
11. Ten Linux returns precede that unsupported request. Sequential offset 832
is preserved and the local watermark reaches 10. The request is retained and
retired without a Linux return or exit.

Independent verification checks all metadata fields and padding, actual caller
bytes and complete saved frames, source-free metadata, four normal delivery
journals and all six injected fault windows. Every lifetime retires the
dependent pair, completes the independent peer and restores owned resources.
The failed first boot exposed a second positional read beyond the original
three-journal budget. Its evidence remains failed; the corrected four-journal
image received fresh source, emitted and physical qualification.

Six source-provisioning refusal cases, all 29 required/affected checks and final
CPU workspace image restoration pass. All five development baseline images
remain byte-identical, with no observed OOM or pressure abort. VirtualBox retains
its CPU capability refusal before userspace and verified poweroff. The
[Open File Metadata Log](../../notebook/Development/2026-10-13%20Open%20File%20Metadata%20Log.md)
retains exact artifacts, failed attempts, reviews and audit receipts. Runtime
mapping, libc startup, completion of `true` and Antimatter remain unqualified.
