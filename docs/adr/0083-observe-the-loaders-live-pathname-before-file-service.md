# ADR 0083: Observe the loader's live pathname before file service

- Status: Accepted for the bounded live-path observation proof
- Date: 2026-10-11
- Builds on: ADRs 0073 and 0079–0082

## Context

The unchanged Linux interpreter receives three actual replies under ADR 0082:
the declared break and explicit absence of the preload and cache files. Its
fourth syscall is another read-only/CLOEXEC `openat`. Unlike the two literal
pathnames, its pathname pointer lies in the caller's writable stack. Initial
ELF/source bytes cannot reveal what the interpreter wrote there. The next file
service must follow an authenticated observation rather than guess a library
name from dependencies or a shared syscall instruction.

The target remains the existing Linux Antimatter/Electron application. An
unchanged dynamically linked `true` completing startup and exiting remains the
nearer execution gate. This increment observes a request; it does not implement
a file open or advance the interpreter past that request.

## Decision

Extend the same default-off `linux_loader_entry_test` profile to copy the live
fourth pathname, then retire as before without a fourth Linux return. Keep the
original executable/interpreter, native mapping plan, source snapshot, initial
stack, private v3 policy bytes and three-task CPU/TLS ownership. Introduce no
native operation, stable file format or namespace answer.

The native veil validates the measured `openat` subset: syscall 257, signed
low-32-bit dirfd `-100` and low-32-bit flags `0x80000`. It copies one byte at a
time through operation 18 under the fourth pending invocation. Checked address
increments cannot wrap. Stop at the first NUL and count that byte; empty or
non-UTF8 contents remain valid observations, with no pathname interpretation.
Malformed shape, inaccessible memory, resource refusal, address overflow and
exhausted work remain distinct pure-reader errors. A failed capture takes the
existing attributed native failure path, never successful retirement or a
fabricated Linux error. Physical qualification covers the observed valid case,
not every pure-reader error branch.

Repeat the six negative native copy probes for this fresh request before the
successful reads: zero/stale sequence, zero length, reserved argument,
inaccessible source and read-only destination. Preserve exact failure results
and the existing writable sentinel checks. Native admission still requires the
exact current veil/frame/root/TSS and sealed suspended-caller authority. Every
attempt preserves all twenty-two saved caller-frame words, the copied request
and pending sequence. Full source/destination preflight, failure atomicity and
the volatile 4096-byte scratch wipe/readback remain required.

Snapshot the blocked caller frame before the first fourth-request copy attempt,
including the negative probes. Compare it on every subsequent attempt and at
terminal completion, before consuming pending authority. Report equal complete
before/after frames. Retain exactly three pathname journals for sequences 2, 3
and 4, reusing the former empty break-reply slot. The first reply reports an
explicit empty journal. Archive the final copy and clear/read back active state
before retiring the caller and veil. Clear/read back all retained pathname
journals after terminal restoration; returned evidence snapshots deliberately
retain their diagnostic copies for reporting.

The 256-byte reader/journal work budget is a removable proof bound, not a global
pathname or depth ceiling. Capture grants neither namespace identity nor file,
descriptor, mapping or credential authority.

## Qualification and limits

Require independent source and fresh compiled stack/control reviews before
physical execution. The extra terminal frame pair adds 352 bytes to the returned
observation; the preceding bootstrap reserve must be remeasured rather than
assumed. Shared entry, eager CPU and TLS ordering remain part of the review.

The oracle requires eighteen ordered records: eight boot events, ready, four
boundaries, three committed replies, one terminal-copy record and retirement.
The three existing complete RAX-only reply pairs and all four interpreter
syscall sites retain their prior checks. Bind the final journal to sequence 4
and the actual request pointer, require an exact unique owned RW/NX stack
mapping and a checked complete range, canonical byte encoding, and exactly one
NUL at the end. Its contents are runtime evidence; do not compare them to the
original source or an expected library name. The terminal frame pair must be
equal and fully correlated with the fourth boundary. Diagnostic fields do not
replace actual sealed authority.

For `N` copied bytes including NUL, require four raw calls, three returns,
`65 + N` native calls, `54 + N` copy attempts, `36 + N` successes and eighteen
rejections. All three tasks retire and source/mapping/CPU/TLS accounting is
restored. Exercise hostile reader, phase and transcript inputs, TCG/KVM at
96/320 MiB, malformed source provisioning and VirtualBox capability gates. Run
all required/affected checks and the retained-evidence audit, compare all five
development-image baselines, and serialize builds/VMs under the unchanged
memory guard.

Qualification completed on 2026-10-11. All four TCG/KVM 96/320 MiB runs
captured `/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libc.so.6`, including
its terminating NUL (55 bytes). Complete terminal frames remained equal; each
run reported 120 native calls, 109 copy attempts, 91 successes and eighteen
rejections, with three actual Linux returns and complete retirement. Path
equality across these runs is measured evidence, not a CPU-independent search
order requirement or a namespace declaration.

All 29 required/affected targets, three negative provisioning cases and the
final evidence audit passed. All five development images remained byte-identical
and all 482 frozen source files remained unchanged. No OOM or pressure abort
was observed. VirtualBox refused the existing
`application_cpu_amd_error_pointers` gate before userspace and powered off; it
does not qualify as a semantic pass. The
[Linux Loader Live Path Log](../../notebook/Development/2026-10-11%20Linux%20Loader%20Live%20Path%20Log.md)
records exact artifacts, emitted stack limits, the corrected focused lint
failure, final audit and memory-observation limits.

Successful file lookup remains future work. It needs an explicit userspace
namespace binding to a pinned held file, task-bound descriptors and subsequent
read/metadata/map/close semantics. Dependency loading, address-space growth,
TLS/libc startup, a Linux exit and Antimatter launch/rendering remain open.
