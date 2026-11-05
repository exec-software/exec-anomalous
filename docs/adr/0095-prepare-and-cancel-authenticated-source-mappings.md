# ADR 0095: Prepare and cancel authenticated source mappings

- Status: Accepted; authenticated unpublished preparation/cancellation qualified
- Date: 2026-10-15
- Builds on: ADRs 0079, 0089, 0090 and 0091

## Context

The loader can reacquire the frame allocator from an authenticated native service
while the Linux caller is suspended. The detached runtime mapping owner already
retains allocation and cancellation obligations, but its geometry and borrowed
backing value confer no task or source authority. Connecting these mechanisms
must precede publication into a live caller's address space.

## Decision

Add a feature-private native source-mapping proof operation, number 25. Its six
arguments are the pending invocation sequence, source grant, virtual base, raw
byte length, populated byte length and native permissions. The first contract
supports a source prefix beginning at offset zero and user-readable, read-only,
nonexecutable pages. It has explicit 2,048-page work budgets, checked arithmetic
and no global address, path or format limit beyond the architecture's address
validation. Populated length must cover the available source through the final
requested page, including valid bytes past the raw request within that page.

The Linux veil alone recognizes the bounded Linux request, checks its live file
binding and translates it into native geometry. The kernel decodes no syscall
number, Linux flags, descriptor policy or ELF structure. Copied native fields
remain untrusted input: the existing sealed scheduler authority must authenticate
the running veil and exact suspended caller, their generations, circles, roots,
privilege stack and saved frame. Bind the source grant to the actual retained
immutable reader, including its provisioned origin and length. Preserve both
the invocation sequence and the separate scheduler correlation words.

Construct the candidate against the caller's actual retained mapping plan.
Collision checks include the entire rounded extent, including absent tail pages
and existing reservations. Reuse the execution owner's independently live
physical alias. Keep the lexical candidate and genuine reader borrowed until
every acquired lease is released; do not fabricate a static source lifetime or
store a self-referential owner. Scratch and readback buffers reside in retained
kernel storage rather than on the privileged stack.

Prepare only an isolated inactive tree. Copy and independently read back every
source byte, verify final-page zero padding, and independently walk every backed
and absent page and both neighboring pages. Every followed table pointer must
belong to the candidate's retained table arena. Compare allocator accounting and
the caller's unchanged tree before and after cancellation. Fingerprint every bit
of its inactive owned table arena; shared alias descendants outside that arena
are excluded. The fingerprint is diagnostic, not an authority token. Preparation failures
remain cleanup-only. Cleanup failure retains the same local owner for an
explicit retry; if cleanup cannot complete, halt without returning or unwinding
past that owner. No dispatch, root selection, external table reference or reply
commit can occur while temporary resources remain owned.

A successful native result reports only the backed and complete virtual page
counts after cancellation. It returns no usable address, mapping handle or Linux
result. Replaying the proof in the same lifetime is refused before allocation.
The veil then uses the existing unsupported-operation retirement, leaving the
first libc `mmap` unanswered after ten Linux returns.

## Qualification boundary

Host tests cover malformed requests, userspace policy translation, full source
and zero-tail contents, independent walks, collisions and budget limits, and
failure retention/cleanup. Native rejection probes must preserve the pending
caller, source and allocator before one successful prepare/cancel and a rejected
replay. Keep the earlier two allocator borrows/one nested denial as distinct
evidence, not a substitute for actual mapping allocation and cleanup.

Require fresh native and kernel emitted reviews, including constructor copies,
all population and cleanup paths, BSS residency and privileged stack bounds,
before guest execution. Four TCG/KVM boots at 96/320 MiB must preserve all seven
file-service lifetimes, the existing fault windows and exact Linux boundary.
Strict host observations and a separate physical crosscheck must agree on the
actual source prefix, complete virtual extent, ownership, cancellation and caller
preservation. Run required and affected regressions serially under the unchanged
OOM guard, retain failed attempts, and record VirtualBox separately.

This operation does not qualify mapping publication, combined caller/root/copy/
circle ownership, TLB withdrawal, descriptor closure across a persistent mapping,
dynamic linking, Linux exit or Antimatter execution. Those remain later steps.

## Qualification on 2026-10-15

Accepted for code `3e3239cd629d9aafd007a6d077e31c843993387b` and its complete
563-file non-Markdown source inventory. Fresh kernel and native emitted reviews
cover the changed allocation, population, cancellation and return paths; no
preceding native-image equivalence approval substitutes for this review.

Four passing TCG/KVM boots at 96/320 MiB preserve all seven file-service
lifetimes. At the normal suspended boundary, one preparation and nine refusals
include a rejected replay. The private candidate at 4 GiB has 535 populated
pages, a 545-page virtual extent, five table frames, 752 zero-padding bytes and
ten absent tail pages. Cancellation restores accounting and preserves the
caller; the six fault lifetimes perform no mapping work. The older two allocator
borrows and one nested denial remain separate observations.

The strict oracle and independent physical crosscheck agree, all thirty
required/affected regression jobs pass, and all six provisioning refusals pass.
The original KVM guest timeout and regression outer deadline remain failed
attempts. VirtualBox separately refuses the CPU capability gate and is verified
powered off; it has no loader-execution pass. The
[Live Mapping Preparation Log](../../notebook/Development/2026-10-15%20Live%20Mapping%20Preparation%20Log.md)
records the exact reviews, artifacts, attempts and final memory audit.
Four development baseline images remain byte-identical; the editor image has
four independently verified loaded panic-line changes and two derived checksum
changes. Its whole-image equality is not claimed.

Sequence 11 remains unanswered after ten Linux returns. The next gate is
persistent ownership and coherent publication across the caller plan, selected
root, circle resources and user-copy validation, followed by withdrawal and
translation invalidation before release. Only that later gate can authorize a
Linux mapping reply and observation of the next interpreter request.
