# ADR 0055: Prove native startup and private userspace arenas

- Status: Proposed; bounded proof qualified by 43 checks and two image restorations
- Date: 2026-10-04
- Related: ADR 0045, ADR 0048, ADR 0050, ADR 0054

## Context

The bounded native editor previously started from compiled-in document state and
static buffers. Antimatter needs supplied document state and owned working memory
before a native runtime can provide real document services. Native mechanisms
remain the reference for later ELF/Linux, Windows and macOS comparisons.

## Decision

Add a removable `native_runtime_test` profile with a copied, versioned 128-byte
native startup descriptor. It supplies role, scenario, private arena extent,
task-local channel handles and bounded initial document bytes. Entry passes its
address and length in RDI and RSI. Decoding rejects unsupported versions,
noncanonical reserved/padding bytes, invalid extents and malformed text. The
descriptor is information, not authority: kernel-owned task generations, channel
bindings, mappings and permissions remain independently authoritative.

Two tasks each receive four retained regions: eight RX code pages, two private
RW data pages, eight private RW/NX arena pages and two stack pages. Only this
explicit trusted preparation profile permits four regions. Existing profiles
retain exact three-region validation and the default three-mapping circle limit.
The explicit arena circle-table constructor permits four mappings, never five. Each arena has unmapped pages immediately
before and after it; code/data/stack reservations remain. Together with private
transition stacks and page tables, the pair owns 66 frames (48 data, 18 table)
in twelve leases. Cleanup remains retryable and must return all ownership.

Userspace borrows the 32 KiB arena through a safe, nonresetting allocator whose
returned mutable slices are disjoint. Actual pointer alignment, arithmetic,
exhaustion and cursor preservation are checked before consuming storage. There
is no mmap, brk, live growth, global allocator, free/realloc, TLS or thread model.
Editor state, pixels and transport buffers use these allocations. Supplied text
initializes a clean saved baseline without changing the preceding editor state
layout or its default fixture. This is not a filesystem open/save service.

Run one unchanged native client binary with two arena layouts, two opaque handle
families and two initial documents. Each task rejects a handle from the other
layout before making progress through its granted handles. Execute normal document one, a guard-write
fault, an NX execution fault, then normal document two. The final fresh launch
checks every arena byte is zero after the preceding fault cleanup. A surviving
service receives closed channels after producer failure. Accepted copy spans
must lie wholly within the selected task's arena, in addition to normal checked
owner and page permissions. No userspace pointer grants access to memory.

Build-time ELF remains a toolchain container for the native artifact. This is
not native Reliquary encoding or foreign ELF startup. Embed the inspected RX
payload and qualified fault offsets; retain the previous native baseline in the
same image. Only the new profile and the preceding shared-veil profile use
kernel section garbage collection. Threshold's 384 KiB staging bound is unchanged.

## Acceptance

Require the original 21 native records followed by exact startup, four scenario,
14 semantic surface and completion records. Compare each document's seven
surfaces and final public pixels with independently asserted semantic checkpoints
and the portable renderer. Rendering is shared cross-build/copy evidence, not an
independent font implementation. Require guard-write and NX page-fault admission,
closed routes, complete resource return and zero observations on every launch.

Qualify 96 MiB and 320 MiB guests and independent captures under TCG/Intel and
KVM/AMD, inspect emitted stack/entry artifacts, run required regressions and
restore development images. Serialize compilers and QEMU, cap build parallelism
at two, and retain memory/OOM observations and failed-attempt evidence.

## Limits

This remains a bounded Rust native editor client and scripted service, not full
Antimatter/Electron. Descriptor documents are at most 31 printable ASCII bytes;
there is no filesystem, asynchronous service discovery, process creation,
persistence or renderer replacement. Ny remains the official language direction.
HAG, Displacer, broad foreign compatibility and production ABI stabilization
remain separate work.

## Qualification receipt

All 43 checks passed on the unchanged frozen set of 340 source/configuration
files, followed by normal and workspace image restoration. Both guest sizes
and independent captures passed on TCG/Intel and KVM/AMD. Captures match and
all twelve preceding artifact blobs remain unchanged. The final kernel and
client match independently reviewed binaries. No host or cgroup OOM occurred.
See the [Native Runtime Log](../../notebook/Development/2026-10-04%20Native%20Runtime%20Log.md)
for exact counts, stack bounds, hashes and preserved failed attempts.
