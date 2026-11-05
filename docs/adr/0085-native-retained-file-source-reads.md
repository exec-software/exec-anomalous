# ADR 0085: Read a retained file source through the native veil

- Status: Accepted bounded native source reads; Linux file-service integration pending
- Date: 2026-10-11
- Builds on: ADRs 0076, 0083 and 0084

## Context

The portable read-only file service needs a bounded guest source backend. The
loader-entry profile currently retains only the executable, interpreter and
startup bytes under a 512 KiB provisioning budget. The pinned libc is 2,190,608
bytes. It must remain alive while userspace consumes it, without creating a
whole-file mapping or putting chunk buffers on small supervisor stacks.

## Decision

Extend only the default-off loader-entry proof. Retain a second immutable owned
source with a separate configured firmware item, grant, length, FNV64 and local
8 MiB budget. The host reads and pins the complete libc with SHA-256. Import both
sources sequentially before execution installs its physical alias. The second
source adds owned storage and one lease; it adds no Linux user mapping and does
not alter the existing native process plan or executable/interpreter bytes.
Firmware names and supplied bytes are provisioning inputs, not authority.
FNV64 is integrity bookkeeping, not cryptographic authentication.

Introduce optional native operation 23 with six words: pending sequence, source
grant, byte offset, destination address, exact byte count and reserved zero.
Authenticate the running veil and its exact suspended caller before every read.
Validate the configured source and checked extent, then preflight the complete
writable veil destination before accessing source bytes. Read 1–4096 bytes using
the execution owner's already-live physical alias and retained kernel scratch.
Never reinstall the bootstrap import alias while tasks run. Do not publish a
failed source read's possibly dirty prefix. Clear the complete scratch on every
return. A successful reply is `(ok, byte_count, 0)`; malformed sequence, extent,
size or reserved words return `invalid`, an unbound source returns `denied`, and
an invalid writable destination returns `bad_copy`, with zero reply payloads.
The source number alone cannot authorize access.

All chunks belong to the same suspended Linux invocation. They neither advance
its sequence nor complete a Linux reply. Preserve its exact request and full
saved frame on successful and rejected source reads. This mechanism carries no
Linux pathname, descriptor, offset or namespace policy into the kernel.

Use a separate 32-byte native deployment descriptor after the existing 128-byte
Linux policy and 8-byte canary. Its four little-endian words are the bytes
`ANFSB001`, source grant, length and FNV64. The original loader veil ELF reserves
zero bytes there; packaging installs trusted facts after inspecting that ELF.
The existing ANLPOL03 bytes retain their meaning. Other veil profiles keep their
136-byte initialized prefix. Linker assertions and host inspection bind this
loader's exact 168-byte initialized prefix and its BSS geometry.

The proof's native service uses a 4096-byte BSS buffer, first consuming 17 bytes
and then bounded chunks through the end. It hashes the bytes returned to its own
address space. A separate 32-byte BSS diagnostic at data offset 176 contains
`ANFSR001`, consumed length, consumed FNV64 and successful chunk count. Before
terminal retirement, the kernel copies this diagnostic from the authenticated
veil, compares it to trusted source facts and its delivery observations, and
retains a compact result for serial reporting. This diagnostic is specific to
the proof; it is neither a completion operation nor application authority.

Keep evidence and scratch in retained BSS instead of growing by-value frame
journals. Retire execution before releasing either source, including cleanup
of the first source if importing the second fails. Require final owned-resource,
CPU, TLS, request and scratch restoration checks.

## Qualification and limits

Require ten rejected-read probes: zero/stale sequence, wrong grant, zero/oversize
length, overflowing/out-of-range source extent, RX destination, a writable-prefix
extent crossing into unmapped memory, and nonzero reserved word. Rejections
preserve the native buffer and destination canaries. Require independent host
comparison of configured source, delivered bytes and guest-consumed length/hash,
including source page crossings and the final tail. Missing, truncated and
corrupt second-source provisioning must refuse before userspace entry.

Before boots, review the emitted native and kernel stack/control paths. Run
required and affected regressions, TCG/KVM at 96/320 MiB, and a VirtualBox attempt
under the unchanged serialized memory guard. Preserve attempted failures and
bind successful evidence to the exact source and artifacts.

Acceptance does not add a Linux file descriptor or a fourth Linux return. The
unchanged interpreter still receives three replies, then remains suspended
while the source proof runs and is retired. Next work must join a chunk-backed
version of the portable file service to authenticated caller/service identities,
staged caller copyout, reply acknowledgement and dependent-caller containment.
General filesystems, stat/mmap, dependency startup, Linux exit and Antimatter
execution remain unqualified. File and transfer budgets here are local proof
resources, not global filename, path-depth or filesystem limits.

## Qualified checkpoint

TCG and KVM passed at both 96 and 320 MiB. Each run consumed the complete pinned
libc in 536 native reads, rejected ten invalid probes and preserved the fourth
Linux invocation without replying to it. Delivered and userspace-consumed
lengths/hashes agreed, and final source, scratch, request, TLS, CPU and owned
resource restoration checks passed. All six missing/size/hash provisioning
cases refused before userspace entry.

All 29 required and affected commands passed on the frozen source, including
the full check. Fresh emitted reviews closed the bounded native/kernel stack
and control paths before boots. Four baseline images stayed byte-identical;
the editor image changed only three diagnostic source-line fields and the
corresponding Threshold checksums, with an independent exact binary review.
An earlier pressure spike triggered the unchanged guard; the complete retry
passed and no OOM kill was observed. VirtualBox still refused its existing CPU
capability gate and powered off. The
[Native File Source Log](../../notebook/Development/2026-10-11%20Native%20File%20Source%20Log.md)
retains failures, exact evidence bindings and observation limits.

Acceptance covers native source consumption. Linux file publication, a fourth
return, dependency initialization, program exit and Antimatter startup remain
unqualified.
