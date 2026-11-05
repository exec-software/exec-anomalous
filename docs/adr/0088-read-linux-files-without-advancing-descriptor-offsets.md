# ADR 0088: Read Linux files without advancing descriptor offsets

- Status: Accepted for the bounded positional-read integration
- Date: 2026-10-13
- Builds on: ADRs 0084, 0086 and 0087

## Context

The unchanged Linux interpreter opens libc and reads its first 832 bytes. Its
next request is `pread64(fd=3, count=840, offset=64)`, which asks for the ELF
program headers. A positional read uses an explicit source offset without
changing the descriptor's sequential offset. This is also the behavior specified
by the [Linux pread manual](https://man7.org/linux/man-pages/man2/pread.2.html).

## Decision

Extend the existing portable immutable-file service with positional reads for
both slice-backed and bounded-source clients. Share validation, extent planning,
publication, scratch clearing and failure handling with sequential reads. The
publication reports the explicit source offset; confirmed publication commits
the request sequence while retaining the descriptor's previous offset. Rejected
publication leaves local state unchanged. Unknown publication or unwinding
fences the service and revokes its descriptors. Provider failures never publish.

The native Linux adapter admits x86-64 syscall 17 with a nonnegative signed
offset and reuses native operations 23, 19 and 20 for source reads, staging and
delayed acknowledgement. Linux syscall interpretation stays in userspace. The
kernel neither derives file offsets nor interprets ELF program headers.

The removable proof retains two independent delivery journals in kernel BSS.
Select the second only for a later sealed request after the first copyout is
committed and the live stage is cleared. Admit journal capacity before source
I/O. Preserve each delivery's source tuple, original caller bytes, staged bytes
and actual post-publication readback. Journals must not become large by-value
stack temporaries. These two slots and 4 KiB transfers are explicit proof work
budgets, not filesystem size or global pathname limits.

## Qualification boundary

Run five fresh lifetimes: normal sequential and positional delivery, the existing
two sequential-read faults, and corresponding positional-read faults. A fault
after positional staging leaves its destination unchanged and retains sequential
offset 832. An injected unknown local result after accepted positional delivery
fences the service without replaying the read. In every case the dependent pair
retires, an independent peer completes, and owned resources are restored.

The independent host oracle derives each delivery's bytes and extent from the
pinned libc and actual syscall arguments. Normal completion requires successful
open, sequential read and positional read before a genuinely unsupported next
request. It must reject retirement on a supported read, including zero-length or
EOF positional reads. Fresh emitted control/stack and semantic reviews precede
ordinary guest execution. Required and affected regressions remain serial under
the unchanged memory guard.

This does not qualify libc mapping, dynamic linking, completion of `true`, general
Linux file operations or Antimatter execution. The next unsupported request must
be observed from the real interpreter rather than prescribed by the test.

## Accepted evidence

Identical reviewed image bytes pass TCG and KVM at 96/320 MiB across all five
fresh lifetimes. The normal unchanged interpreter receives 840 bytes from libc
offset 64 and then requests `fstat(fd=3)`, syscall 5, after nine calls and eight
Linux returns. Its sequential offset remains 832. Separate positional fault
cases distinguish unchanged caller memory after staging from accepted copyout
followed by an injected unknown local result and service fencing. Every lifetime
retires the dependent pair, completes the independent peer and restores ownership.

Six source-provisioning refusal cases, all 29 required/affected checks and final
CPU workspace image restoration pass. All five development baseline images
remain byte-identical. No OOM kill, hard-limit event or pressure abort is observed.
VirtualBox's CPU capability refusal before userspace and verified poweroff remain
separate from successful QEMU qualification. The
[Positional File Reads Log](../../notebook/Development/2026-10-13%20Positional%20File%20Reads%20Log.md)
retains exact source/artifact identities, fresh emitted reviews, physical
transcripts, earlier failed attempts and final audit receipts.
