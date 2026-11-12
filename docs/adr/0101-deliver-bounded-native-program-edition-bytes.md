# ADR 0101: Deliver bounded native program edition bytes

- Status: Accepted; bounded TCG/KVM qualification completed 2026-11-11
- Date: 2026-11-11
- Builds on: ADRs 0048, 0076, 0079, 0091, 0093, 0094 and 0100
- Scope: Experimental read access to one controller-bound immutable edition.
- Qualification: Thirty final jobs passed on the frozen 598-file source set.

## Context

[ADR 0100](0100-qualify-native-program-console-transport.md) connects the ANPG
program profile to a checked console service. Its edition handle remains inert.
The [Runtime Owner Reply](../../notebook/Development/2026-10-15%20NIL%20Runtime%20into%20Anomalous%20%E2%80%94%20Runtime%20Owner%20Reply.md)
selects a file service as the edition source so that changing a program does not
require rebuilding the interpreter image.

The kernel already owns copied, sealed sources under
[ADR 0076](0076-prove-owned-loader-sources.md), with a distinct privileged
provisioning origin under
[ADR 0079](0079-reach-an-unchanged-linux-interpreter-service-boundary.md).
Those mechanisms retain immutable bytes, bounded physical access and explicit
cleanup. They do not authenticate an executable or authorize interpreting it.

This slice connects that source ownership to a native read operation. The
fixture carries synthetic binary bytes, not a NIL program. Semantic-manifest
derivation and admission under
[ADR 0093](0093-carry-a-semantic-manifest-in-the-program-image.md), followed by
interpreter qualification, remain separate gates before edition execution.

## Decision

### Bind one edition to the existing program launch

Keep the 256-byte ANPG encoding, one-task profile and 64 KiB arena. Expand only
the named program profile to two RX pages: the edition-delivery client's measured
payload is 5,605 bytes and its admitted padded image is 8,192 bytes. The generic
page-count interface still accepts its existing one/eight-page shapes; two pages
require the retained program selection and exact single-task geometry. The
native application's eight RX pages and 32 KiB arena remain unchanged. This
bounded fixture sizing does not select an interpreter's eventual geometry.

The descriptor's `file_read` service bit permits its typed `edition()` accessor
to return the edition handle. Without that bit, edition metadata alone supplies
no usable typed handle. The live kernel binding remains authoritative even if
userspace rewrites its descriptor or guesses a previously valid number.

The controller binds the exact task identity, optional handle, source length
and admitted arena before execution is published. This handle names one retained
snapshot. It does not name a filesystem, path prefix, descriptor table or
general-purpose file service. The console and edition have separate bindings
and sequence state.

The portable [file-service model](../../crates/anomalous-veil-protocol/src/file_service.rs)
already supplies namespace views, descriptor generations, positions and
publication acknowledgement for userspace adapters. Importing that model into
the kernel would introduce path and descriptor policy that this launch does not
need. Reuse the sealed-source owner and its physical-copy mechanisms instead;
keep namespace and foreign descriptor policy outside this native boundary.

### Add a bounded operation to the experimental gate

Use operation 2 on the existing `int 0xf4` native program gate. Startup 0,
console write 1 and exit 5 retain their contracts. This is an experimental
profile-local register contract, separate from the `int 0xf3` rite vocabulary.

| Register | Edition-read request | Edition-read reply |
| --- | --- | --- |
| RAX | Operation 2 | Status |
| RDI | Raw edition handle | Preserved |
| RSI | Destination address in the arena | Preserved |
| RDX | Capacity, from 1 through 4,096 bytes | Delivered byte length, or zero on rejection |
| RCX | Exact next request sequence | Committed sequence, or zero on rejection |
| R8 | Unsigned source offset | Preserved |

Sequences start at one. Reject zero, gaps, replay and exhaustion. Validate
checked source-offset plus capacity arithmetic; overflow is `invalid`, including
requests that would otherwise be beyond EOF. For a valid extent calculation,
deliver `min(capacity, source_length - offset)` bytes when the offset is before
EOF. An offset at or beyond EOF returns a successful zero-byte read and consumes
the sequence. The read has an explicit offset and no shared file position.

Status values are `ok = 0`, `denied = 2`, `bad_address = 3`, `invalid = 4`,
`closed = 5` and `unavailable = 6`. The last value reports a failed source read
or a provider response whose length differs from the planned transfer. This
extends only the experimental `program_status` vocabulary.

### Preflight the complete destination before source access

After the gate validates its live task, saved frame, root and retained owners,
check identity, handle, sequence, capacity and source arithmetic. Require the
entire requested capacity to fit the admitted arena using checked address
arithmetic. Preflight that complete writable destination through
[owned execution](../../kernel/src/arch/x86_64/owned_execution.rs), even for a
short final read or EOF. An unused tail cannot conceal an invalid destination.

Read only the planned nonempty source extent through a borrowed
`sealed_source_reader` and the execution owner's existing physical alias.
Stage the result in a 4,096-byte supervisor BSS scratch buffer. A provider may
dirty scratch before returning an error; none of those bytes may be published.
Require the exact planned length before copying to userspace. Edition bytes are
binary and do not use the console's UTF-8 or control-character restrictions.

The publication callback copies only delivered bytes; the remaining destination
capacity stays unchanged. A returned publication error guarantees that no
destination byte changed. The physical copy repeats full permission and extent
checks before its first write; a privileged hardware fault remains fatal and
cannot be presented as a retryable partial-copy failure.

For nonempty reads, commit sequence and byte accounting only after publication
succeeds. Rejected requests consume no sequence and change no destination bytes.
Clear scratch on entry and every returning path. These are logical clearing
guarantees, not an erasure guarantee following an abort. EOF performs no source
read or copyout; it commits its successful receipt after the complete destination
preflight.

### Reuse explicit source provisioning and teardown

The default-off QEMU proof uses
[`source_controller::new_configured`](../../kernel/src/arch/x86_64/linux_loader_source.rs)
with the exact fw_cfg item `opt/anomalous-program-edition`. Trusted linked
metadata fixes an 8,195-byte source and an equally bounded source budget.
Byte `i` is `(i * 37 + 11) mod 256`; its FNV-1a-64 value is
`0x84ca7df1efd6b401`. FNV detects accidental corruption and supports evidence
comparison; it is neither authentication nor semantic-manifest identity.

The source arrives separately from the native client image. The configured
controller validates the bounded directory, exact item, exact length and hash,
copies and reads back three chunks, and seals the retained source before making
it readable. No task identity is fabricated for privileged provisioning.

Retain the controller in initialized BSS before import. Finish import and remove
its temporary alias before preparing execution: both owners use the same high
physical-alias slot. During service calls, the source reader borrows the actual
sealed owner and the execution owner supplies the independently live alias.
Generalizing that read helper to a const-generic task group does not grant a
source, authenticate a caller or change the Linux three-task preparation path.

The granted scenario adds three data frames and one bound source lease to the
task's 26 data frames, nine table frames and six leases. The task's data frames
comprise two code, two data, sixteen arena, two user-stack and four transition
frames. Keep source accounting separate: the granted scenario holds 29 data
frames, nine table frames and seven leases in total; the ungranted scenario
imports no source.
On exit, revoke both service bindings, retire the task and finish execution,
then finish the source through its independent cleanup alias. Partial owners
remain retained after errors until explicit cleanup succeeds.

Missing input, incorrect length and a same-length hash mismatch are rejected
before task publication. The proof must finish any partial source, verify its
release and exact allocator restoration, then emit its attributed rejection
record before terminating with the expected failure status.

### End preparation borrows before guest transfer

Apply [ADR 0091](0091-release-allocator-borrows-before-userspace.md) to the
native program profile. Source import, task preparation, scheduler admission
and execution activation occur within a synchronous allocator callback. That
callback returns before initial task selection, active-dispatch publication or
guest entry. Only copied accounting and raw pointers to retained BSS/frame
storage cross the transfer; no allocator borrow or mutable program reference
may remain live across it.

Terminal dispatch restores the bootstrap root, privilege stack and CPU state
and withdraws active dispatch before returning. Validate the resumed
continuation before acquiring a fresh allocator borrow for withdrawal and
cleanup. That borrow finishes execution before source cleanup, preserving the
alias and ownership ordering above. The allocator's exclusive entry point is
unchanged; no saved allocator pointer bypasses it.

Once in the granted lifetime, after the first authenticated edition request's
complete destination preflight, acquire the allocator through its ordinary
entry point and compare the prepared live accounting. Require one nested borrow
to return `allocator_busy` without entering its callback, then release the outer
borrow and acquire normally again. Preserve the complete saved frame and request.
This probe observes metadata without allocating or publishing a mapping. Require
two sequential service borrows and one nested denial in the granted scenario,
and none in the ungranted scenario. The qualification below records those
physical observations; a successful host test alone cannot prove this hardware
dispatch boundary.

## Acceptance contract

Keep both fresh task lifetimes and arena bases, `0x804000` and `0x818000`.
The first grants console and edition reads; the second grants neither. Preserve
the two exact console lines and their checked replies from ADR 0100.

| Evidence | Granted scenario | Ungranted scenario |
| --- | ---: | ---: |
| Total gate calls, including startup and exit | 39 | 7 |
| Console attempts / writes / rejections | 16 / 2 / 14 | 2 / 0 / 2 |
| Console bytes | 60 | 0 |
| Edition attempts / committed reads / rejections | 21 / 7 / 14 | 3 / 0 / 3 |
| Delivered edition bytes | 8,203 | 0 |
| Nonempty source reads | 5 | 0 |
| Ordinary allocator service borrows | 2 | 0 |
| Denied nested allocator borrows | 1 | 0 |

The seven committed reads include two zero-byte EOF replies. Delivered-byte
accounting includes repeated reads and therefore exceeds the source length.
Require exact receipts and byte verification in the native client, unchanged
canaries on rejected requests and after delivered prefixes, and successful
unaligned copies across page boundaries. Host tests must cover wrong task
incarnations and handles, sequence and arithmetic edges, full-capacity
preflight, dirty/short provider failure, rejected publication, scratch clearing,
EOF and revocation.

Require restored bootstrap RSP, root, privilege stack, interrupt and CPU state
before teardown, followed by exact allocator restoration and final
`allocator_available = true` evidence. Before writing any edition reply fields,
compare the complete 176-byte saved caller frame with its authenticated entry
snapshot. Require `edition_frame_preserved = true` in the final success record;
this includes both accepted and rejected edition requests.

The strict transcript oracle must distinguish successful delivery from the
three deliberate provisioning refusals. Each refusal requires eight boot
records, one attributed `source_rejected` record with release/restoration true,
then the matching fatal record and exact failure exit status. Fatal evidence
in these negative proofs is not a successful application run.

Qualify the successful profile at 96 MiB and 320 MiB under TCG and host-CPU KVM,
plus the repository-required and affected regressions. Retain exact source and
image identities, failed attempts, host counts and restoration evidence in the
[Edition Read Log](../../notebook/Development/2026-11-11%20Native%20Program%20Edition%20Read%20Log.md).

## Qualification on 2026-11-11

All thirty final jobs pass on 598 unchanged tracked and untracked non-Markdown
source files. The final verifier completed at `2026-11-12T00:19:30Z`, still
November 11 in the development timezone. It records 644 default-feature and
727 all-feature kernel tests, 402 xtask tests and 35 native-runtime tests;
the focused program and tooling selections pass 24 and 12 tests respectively.

Identical 16 MiB images pass native delivery at 96 MiB and 320 MiB under both
TCG/Nehalem and KVM/host. Every successful boot meets the complete per-scenario
contract above, including frame preservation, allocator reacquisition, binding
revocation and resource restoration. Each backend additionally refuses missing,
short and corrupted source input at 96 MiB after proving source release and
allocator restoration. All five KVM command arrays are retained; the three
deliberate failure exits are separate from the two successful native runs.

The emitted review is bound to the exact tested images. Their SHA-256 is
`e611655bf95c6fb4da0b402dd83dc875c3069788d2c299a80ba55d39bd54cb2a`.
The reviewed 143,360-byte kernel occurs exactly once at image offset `0xe00`;
the reviewed 5,605-byte client matches its ELF and the kernel payload, including
the R8 offset transport and complete reply stub. Source and emitted inspection
confirm preparation-borrow release before entry and execution retirement before
source cleanup. This review does not claim an exhaustive stack-bound,
reference-aliasing or call-graph proof.

The initial one-page link refusal and the subsequent preliminary run remain
retained outside final qualification. The measured client size justified the
explicit two-page program selection above; it did not widen generic admission.
The final matrix has no failed attempts. This checkpoint qualifies the bounded
QEMU TCG/KVM profile; no VirtualBox run was made for this slice.

## Limits and subsequent gates

Keep `edition_mapped = false` and `editions_executed = 0`: selected bytes are
copied into ordinary writable arena memory, and the source receives no direct
user mapping or executable permission. The fixture does not interpret NIL or
assert a semantic manifest. The selected Rust interpreter still needs the
unmodified execution corpus, and ADR 0093 still requires artifact-derived
authority, intent, identity and loader admission before execution.

This decision adds no pathname lookup, general file-open API, filesystem,
asynchronous I/O, time service, Linux syscall behavior or stable public ABI.
