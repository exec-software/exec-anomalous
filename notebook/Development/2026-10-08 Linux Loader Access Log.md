---
title: "Linux Loader Access Log"
date: 2026-10-08
tags: [linux, elf, userspace, loader, copy, namespace, qualification]
status: qualified-bounded-proof
---

# Linux Loader Access Log

The next bounded interpreter increment answers the measured
`access("/etc/ld.so.preload", R_OK)` request from an explicit userspace absence
declaration, after the qualified `brk(0)` reply. The target remains the unchanged
Linux Antimatter/Electron application. This does not establish a general
filesystem or complete a Linux process.
[ADR 0081](../../docs/adr/0081-answer-an-unchanged-loader-file-probe-in-userspace.md)
defines the increment; the
[Linux Loader Reply Log](2026-10-08%20Linux%20Loader%20Reply%20Log.md)
preserves the preceding two-boundary result.

## Implemented policy and copying

The private 128-byte policy record is versioned as `ANLPOL02`, with the
plan-derived initial break and a declaration that exactly `/etc/ld.so.preload`
is absent in the initial personality namespace view. It is supplied explicitly,
not inferred from the host filesystem. Host construction/validation and the
feature-gated userspace decoder independently bind every byte. The kernel
writes only the existing task role word and does not parse namespace policy.

The userspace policy reads one byte per existing native operation 18, stops at
NUL and checks address arithmetic. Only the declared complete name yields
`-ENOENT`; the pure policy also distinguishes inaccessible memory (`-EFAULT`)
and invalid Linux integer mode bits (`-EINVAL`). Unknown names, unsupported
operations and exhausted work remain unsupported. A 256-byte invocation budget
and the controller's 256-byte diagnostic journal are removable proof bounds,
not global limits on native pathname length or depth. This profile qualifies
only the actual valid preload probe, not the other pure-policy branches as
physical Linux returns.

The native endpoint admits an exact current veil/frame/root/TSS and sealed
pending caller with nonzero correlation, reserved-zero arguments and bounded
length. Existing owned-copy APIs preflight inactive caller storage and current
veil output before modifying destinations. A retained 4096-byte BSS scratch is
volatile-cleared and read back after every returned outcome. The bounded
contiguous-copy journal is appended only after successful output. Around every
attempt the controller compares all twenty-two blocked caller-frame words and
the copied request, and revalidates pending authority and sequence.

Six native negative probes cover zero/stale correlation, zero length, nonzero
reserved argument, inaccessible source and a read-only destination. The veil
checks exact native errors and its unchanged 24-element `i32` sentinel
(96 bytes, explicitly observed after every probe). Read-only destination denial
also depends on the existing actual-PTE permission preflight. Nineteen successful
byte copies include NUL. The second reply changes only saved RAX; a fresh third
syscall must demonstrate actual continued loader execution before retirement.
Unsupported second-request policy is an attributable failed proof, not an
alternate successful terminal outcome.

## Physical observations

TCG/Nehalem and KVM/host passed at both 96 and 320 MiB, with identical retained
kernel/image/source/plan/policy bytes. Each transcript has fifteen ordered
records: eight boot events, ready, three raw boundaries, two complete reply
frame pairs and terminal restoration. Both replies change only RAX. The second
and third actual traps independently demonstrate both caller continuations.

| Observation | First boundary | Second boundary | Third boundary |
| --- | --- | --- | --- |
| Sequence | 1 | 2 | 3 |
| Linux operation | 12, `brk(0)` | 21, `access` | 257, `openat` |
| SYSCALL instruction | `0x600284c9` | `0x60029489` | `0x60029743` |
| Next RIP / RCX | `0x600284cb` | `0x6002948b` | `0x60029745` |
| Saved RSP | `0x7fff0000fda8` | `0x7fff0000fae8` | `0x7fff0000eec0` |
| R11 | `0x246` | `0x246` | `0x206` |
| Userspace result | `0x40009000` | `-ENOENT` (`-2`) | unsupported `-38` bits |
| Returned to Linux | true | true | false |

All FS bases remain zero, and each instruction resolves to `0f 05` in the
retained unchanged interpreter. The copied journal contains exactly nineteen
bytes, `/etc/ld.so.preload` including NUL, from caller address `0x60033720`.
The host oracle independently checks the initialized read-only/nonexecutable
source span and compares the actual copied bytes with the policy declaration.
Each run records 34 native calls, 25 copy attempts, nineteen successes, six
rejections, cleared scratch, three retired tasks and complete source/mapping/
TLS/CPU restoration. All 223 owned frames are returned, including 71 source
pages and the execution owner's 124 data plus 28 table frames.

The third raw request is
`openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY | O_CLOEXEC, 0)`.
Its raw dirfd is `0xffffff9c` (signed low-32-bit `-100`), pathname pointer
`0x60032491`, flags `0x80000`, and zero remaining arguments. The exact seventeen
pathname bytes including NUL occupy interpreter file offsets
`0x32491..0x324a2` and retained-source offsets `0x3ae69..0x3ae7a`, in initialized
read-only PHDR 2. This pathname is reconstructed from retained source; the
guest did not copy that third pathname or perform a successful open. Retirement
does not return the third result to Linux, and no main `true` execution or
Linux process exit is claimed.

Missing, one-byte-short and one-byte-corrupted source inputs were rejected in
three 96 MiB TCG boots with `linux_source_missing`, `linux_source_size` and
`linux_source_hash`. Each stopped at eight boot records plus its exact fatal
event, exit 35 and no caller boundary. This proves source rejection, not
independently measured negative-path cleanup accounting.

VirtualBox 7.2.6 attempted the same image at 320 MiB, refused
`application_cpu_amd_error_pointers` before userspace, and powered off cleanly.
This is a capability refusal, not a guest execution pass. Source provisioning
still uses the QEMU-specific interface.

## Source and emitted review

All 482 non-Markdown source files were frozen before focused validation.
Independent reviews covered sealed copy authority, complete preflight, error
atomicity, scratch clearing, journal admission, both reply-frame commits and
the independent transcript oracle. These are returned-error and bounded-runtime
claims, not fault recovery from arbitrary privileged exceptions.

The fresh kernel graph closes 73 register-call sites and six indexed tables.
Bootstrap stack use is 40,752/65,536 bytes, leaving 24,784 bytes and 8,400 bytes
beyond the separate 16 KiB reserve. Including the 176-byte saved frame,
SYSCALL/native/timer bounds are 5,296/5,184/3,296 of 16,384 bytes; the bounded
fault prefix is 2,448 bytes. Generic fatal recursion, serial-format vtable
recursion and NMI nesting remain outside the certificate. No 4 KiB copy scratch
is placed on a handler stack. Shared inspected entry/TLS sequences remain
unchanged, and the copy, volatile wipe/readback and RAX-only stores survive
optimization.

The native maximum reviewed stack use is 1,176/8,192 bytes, leaving 7,016 bytes.
The native review binds policy loads, all six rejected probes, immediate NUL
termination, both successful gate results and fresh third-request retirement.
It is not a static bound for the unchanged interpreter or a sustained
preemption/FP-pattern qualification.

Retained focused artifact SHA-256:

- Kernel ELF: `a2c3e6f1c5d42c8d563da5d291d7103c68fed0f44a90867a219e89ad8a5bc6a0`.
- Disk image: `062a23db66317992f8c50b0c26815419b94661b8de80d338ff6b430dcc981d44`.
- Native veil ELF: `7ad0ad270bee87960dc5b688d97ae7e4baed4ab21a496e1de61ecfeb723c6c55`.
- Policy: `2296f474bc2100f6f8a2199c47681a0e43c12abfbc1a82e7de1c92847b74ab45`.
- Native plan: `b971aaf469ff272c81d3fba60e375d8f8d0059d7c46685b7b294ee0e58d5aee1`.
- Source snapshot: `b79d9ccc61e497bd773d199f430d4522278817b82e2d45de60d66219a345af90`.

The main/interpreter, source snapshot and mapping plan remain unchanged from
ADR 0080. Evidence is retained under
an external validation directory (not distributed).
Old thirteen-record logs copied into the focused build bundle are excluded from
new physical evidence; successful new backend logs must be retained separately.
Generated files remain outside Git.

## Final qualification

Implementation checkpoint: `b9100783629376a9090ff12a1ce1d43411801029`
(`Answer unchanged Linux loader preload probe through userspace copy policy`),
pushed to `origin/main` after qualification. This log and readiness updates form
the following documentation checkpoint.

Focused validation passed 611 kernel tests, 345 host-tool tests and nine
policy-crate tests (including seven new policy groups), policy/kernel/host
warning checks, image construction and artifact capture. The twenty-nine-target
regression matrix passed all sixteen required and thirteen affected targets.
The final CPU-workspace baseline rebuild/capture also passed. The final manifest
selects 44 successful guarded jobs across six scopes, including artifact captures;
the separately attempted VirtualBox refusal is not a selected success. All seven
guarded scopes completed with final memory samples and stopped monitors.

Normal, workspace-dev, owned-workspace, CPU-workspace and native-editor images
are byte-identical to their initial baselines. Independent comparison also
checked complete kernel payloads and Threshold geometry/checksums. All 482 frozen
source files remained unchanged.

The independent `final-audit-v1/receipt.json` passed at 16:45:23 UTC, binding the
private policy and deployed native bytes, all three interpreter syscall sites,
both complete reply-frame pairs, the copied pathname, negative inputs, baseline
images, emitted reviews and serialized guarded intervals. Its SHA-256 is
`07843521e1f28563bd1f7e6f9c53ac4e845d521d246657cd940588265ac1b092`.
The selected `final-evidence-v2.json` manifest SHA-256 is
`97f984636f0eae9433678342299e874b68d3f05c76c1bbb91df47a8dab8462f3`.
The earlier unaudited manifest remains retained: it was generated before the
final baseline review finished and was superseded to bind the frozen review.
No audit or qualification failure required a code change.

Builds and VMs ran serially with one worker under the unchanged 4 GiB soft /
6 GiB hard / 512 MiB swap memory guard. Peak scope usage was 4,296,613,888 bytes;
available host memory stayed at or above 12,318,679,040 bytes. The soft-limit
counter advanced during reclaim, while hard-limit and scope OOM counters
remained zero. No pressure abort occurred. The host kernel OOM counter remained
zero through the audit. Accessible unit/kernel journals showed no qualifying
intervention; the oomd query returned no accessible records. Collected units and
missing history remain unavailable, not historical zero. One-second samples
cannot establish continuous pressure observation or reconstruct unlogged final
PSI details. Host swap already existed. The supported conclusion is no observed
OOM or guard abort across the retained work.

## Next observed service

The next boundary is the optional loader-cache `openat` request above. A bounded
next step can explicitly declare that cache path absent in the initial userspace
namespace view, copy the actual third pathname under its pending caller
authority, return the corresponding absence result, then observe the fourth
request. It must not infer absence from the host cache or answer every open with
`-ENOENT`. A successful open requires a real held file source, task-bound
descriptor authority and subsequent read/stat/map semantics. No descriptor or
file contents are supplied by the current proof.

Dependency loading, address-space growth/protection, TLS/libc startup and a real
`true` exit remain open before Electron's larger process and display services.
There is no Antimatter launch or rendering claim.
