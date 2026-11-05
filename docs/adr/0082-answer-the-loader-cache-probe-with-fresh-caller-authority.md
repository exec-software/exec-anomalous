# ADR 0082: Answer the loader cache probe with fresh caller authority

- Status: Accepted for the bounded three-reply proof
- Date: 2026-10-08
- Builds on: ADRs 0053, 0073 and 0079–0081

## Context

The unchanged interpreter now resumes after `brk(0)` and the declared-absent
preload-file probe. Its next measured request is
`openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY | O_CLOEXEC, 0)`.
The existing Linux Antimatter/Electron application remains the product target;
an unchanged dynamically linked `true` completing startup and exiting is the
nearer gate. A loader cache is optional in an explicitly configured initial
namespace. Declaring it absent requires neither fake file contents nor a
descriptor; it does require fresh authenticated copying and a truthful userspace
namespace fact.

## Decision

Advance the same default-off `linux_loader_entry_test` profile to three actual
Linux returns and a fourth observed syscall, then explicit retirement. Keep the
original executable/interpreter, retained source snapshot, native mapping plan,
Linux startup stack and three-task CPU/TLS ownership. No new native operation,
stable artifact format or general filesystem is introduced.

Version the private 128-byte userspace record as `ANLPOL03`, version 3. Preserve
role at 0, size at 24, initial break at 32, namespace kind 1 at 40, preload-path
length 18 at 48 and `/etc/ld.so.preload` at 56..74. Add cache-path length 16 at
80 and `/etc/ld.so.cache` at 88..104. Gaps 74..80 and 104..128 are zero.
Both exact names are explicitly absent in the initial personality namespace;
this does not infer host filesystem state. The host constructs and validates
the complete declaration independently of the userspace decoder. Privileged
code writes only the existing task-role word and does not parse either path.

The userspace `openat` policy admits the measured read-only/CLOEXEC subset:
syscall 257, signed low-32-bit dirfd `AT_FDCWD` and low-32-bit flags `0x80000`.
Mode and unused arguments do not affect this non-creating request. Other dirfds,
flags, operations or names remain unsupported; the subset does not implement
general absolute-path dirfd semantics. Reuse checked one-byte caller reads,
stopping immediately at NUL. Only the exact declared cache name produces
`-ENOENT`; inaccessible or wrapping pointers produce `-EFAULT` in the pure
policy. Resource refusal and exhausted work remain unsupported. Only the
observed valid request is physically qualified, not every pure-policy branch.

Reuse native operation 18 under the new sealed suspended-call authority and
current veil/frame/root/TSS. Retain the existing copy argument validation,
source/destination preflight, failure atomicity and full scratch wipe/readback.
The completed preload invocation's diagnostic bytes must never become cache
request evidence. Archive each reply's observation separately, then clear and
verify the active journal before resuming the caller. Admit a new raw request
only with clear journal and scratch. A nonempty journal must match the current
request sequence and contiguous source address. After terminal restoration,
clear and verify the archived retained journals as well.

Run the six negative native copy probes independently for both pathname calls:
zero/stale correlation, zero length, reserved argument, inaccessible source and
read-only destination. Each checks exact error words and an unchanged writable
sentinel. Read nineteen preload bytes and seventeen cache bytes, including
each NUL. Commit the third authenticated RAX-only `-ENOENT` reply, retaining
the waiting veil's authority and successful gate result. Observe a fresh fourth
request without predicting its syscall or arguments, then retire without
returning its unsupported result bits to Linux.

The 256-byte per-invocation pathname work budget and 256-byte diagnostic journal
remain removable proof bounds, not global native pathname or depth ceilings.
Unsupported policy at an expected-return phase is an attributable failed proof,
not a separately qualified alternate ending.

## Qualification and limits

Require hostile policy, completion/copy-shape and transcript tests, independent
source review, and fresh emitted entry/return/copy/stack review before booting.
The oracle must accept exactly seventeen ordered records: eight boot records,
ready, four raw boundaries, three committed replies and complete retirement.
Independently decode all three complete before/after frames and require only
RAX changed. Bind all four syscall instruction sites to unchanged interpreter
bytes. Bind each copied path separately to its policy declaration, actual
request pointer, sequence and unique initialized read-only/nonexecutable source
span; reject crossed or reused journals even if aggregate counts agree.

Require four raw calls, three returns, 59 native calls, 48 copy attempts,
36 successes, twelve rejections, clear scratch/journals, three retired tasks
and exact source/mapping/TLS/CPU restoration. Exercise TCG/KVM at 96/320 MiB,
malformed source provisioning and VirtualBox under existing capability gates.
Run all required/affected checks and the independent retained-evidence audit.
Serialize builds and VMs under the unchanged memory guard, compare the five
development-image baselines and preserve failed attempts separately.

Qualification completed on 2026-10-08: TCG/KVM at both memory sizes, three
negative source-provisioning cases, all 29 regression targets and the independent
final evidence audit passed on 482 unchanged frozen source files. All five
development-image baselines remained byte-identical. The first presentation
smoke failure and subsequent complete-record capture repair are retained
separately; the complete final matrix passed after a fresh source freeze.
No OOM or pressure abort was observed. VirtualBox refused the existing
`application_cpu_amd_error_pointers` gate and powered off; it is not a semantic
pass. The
[Linux Loader Cache Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Cache%20Log.md)
records the evidence and memory limits.

The fourth actual request is another `openat`, with pathname pointer
`0x7fff0000eec0` in mutable stack memory. It was not copied and its contents
cannot be reconstructed from the original source. The next increment must
authenticate and observe those live bytes before assigning namespace behavior.

This proof supplies no successful open, file descriptor or file data. General
namespace traversal, regular-file read/stat/map/close, heap growth, dependency
loading, TLS/libc startup, a Linux process exit and Antimatter launch/rendering
remain open. The next service follows the fourth actual observed request.

## References

- [Linux v6.17 open implementation](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/open.c).
- [glibc 2.42 dependency loading](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/elf/dl-load.c).
