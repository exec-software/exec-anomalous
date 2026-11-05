---
title: "Linux Loader Cache Log"
date: 2026-10-08
tags: [linux, elf, userspace, loader, copy, namespace, qualification]
status: qualified-bounded-proof
---

# Linux Loader Cache Log

This increment answers the measured
`openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY | O_CLOEXEC, 0)` request from an
explicit userspace absence declaration. The unchanged interpreter resumes
a third time and reaches a fourth actual syscall before retirement. The target
remains the existing Linux Antimatter/Electron build; unchanged dynamically
linked `true` completing startup and exiting is the nearer gate.
[ADR 0082](../../docs/adr/0082-answer-the-loader-cache-probe-with-fresh-caller-authority.md)
defines this increment. The
[Linux Loader Access Log](2026-10-08%20Linux%20Loader%20Access%20Log.md)
preserves the preceding two-return result.

## Policy and fresh copying

The private 128-byte userspace policy becomes `ANLPOL03`, version 3. It retains
the initial break and exact preload-path absence, and adds a cache-path length
of sixteen at offset 80 and `/etc/ld.so.cache` at 88..104. Gaps 74..80 and
104..128 remain zero. Host construction/validation and userspace decoding
independently bind every byte. Neither absence is inferred from the host;
privileged code writes only the task role and does not parse Linux names.

The pure cache policy admits syscall 257, signed low-32-bit dirfd `-100` and
low-32-bit flags `0x80000`. Mode and unused registers are ignored for this
non-creating call. Other dirfds, flags and names remain unsupported; this is a
narrow personality subset, not general Linux pathname resolution. Checked
one-byte reads stop immediately at NUL, including at the last mapped byte.
Only the exact declared cache path yields `-ENOENT`. Pure-policy inaccessible
or overflowing reads yield `-EFAULT`; resource refusal and exhausted work stay
unsupported. The 256-byte invocation budget is not a global pathname limit.

Existing native operation 18 authenticates the new pending caller and the exact
current veil/frame/root/TSS. Full source and destination preflight precedes
publication. The runtime preserves all twenty-two blocked caller-frame words,
the copied request and pending sequence around every read. Its retained
4096-byte scratch is volatile-cleared and read back on every returned outcome.

Each successful reply archives that invocation's diagnostic copy observation
separately. The active journal is then cleared and verified before selecting
the caller; a new syscall also requires clear journal and scratch. Nonempty
journals must match the current sequence and contiguous source address. After
terminal restoration, retained archived journals are also cleared and verified.
Returned serial-evidence snapshots deliberately retain their copies; the
cleared-journal claim does not describe those evidence copies as erased.

Six negative native-copy probes run independently under each pathname request:
zero/stale sequence, zero length, reserved argument, inaccessible source and
read-only destination. Each checks exact error/result words and its unchanged
24-element `i32` sentinel (96 bytes). The read-only destination's no-write
guarantee also relies on actual mapping preflight, rather than the unrelated
stack sentinel alone. Nineteen preload and seventeen cache byte reads include
both NUL terminators. The third reply changes only saved RAX, and the native
veil checks successful completion after the next actual caller trap.

## Qualification

The first regression attempt passed fourteen targets, then the existing
`semantic-presentation-smoke` rejected its transcript. Its fail-closed harness
removed the unverified serial and pixel artifacts; the failed command log and
complete guard evidence remain retained. There was no OOM, pressure abort or
runner error. The deleted transcript prevents a conclusive failure attribution.

Source inspection found a capture race: the waiter recognized the terminal
event substring before the full record was written, then could send QEMU
`quit` early. The host-only repair waits for the final canonical CRLF-terminated
JSON record, exact event/schema/level and typed counters. Existing postcapture
transcript, protected-data and pixel checks remain unchanged. A regression
checks every truncated byte prefix and malformed terminal records. Freeze v2
retains the same 482-file inventory; only the host tool changed from v1. Fresh
focused checks and the complete 29-target matrix passed, with the repaired
presentation check first. The failed attempt remains separate evidence; this
successful rerun does not establish the cause of the deleted failed transcript.

All 482 non-Markdown source files were frozen before focused validation. Two
independent source reviews found no blocker in copy authority, failure
atomicity, per-invocation reset, reply-frame preservation and the transcript
oracle. Final focused validation passed 611 kernel tests, 348 host-tool tests and
fourteen policy-crate tests (twelve policy groups), plus policy/kernel/host
warning checks, image construction and artifact capture. All sixteen required
and thirteen affected regression targets passed on the final frozen source.

The fresh native compiled-artifact review closes all 23 sized text functions
with no indirect branch or dynamic stack adjustment. Its maximum stack path is
1,064/8,192 bytes, leaving 7,128 bytes. Emitted inspection confirms the complete
v3 decoder, both six-probe calls, both immediate NUL-stop loops, low-32-bit
openat argument handling, three checked successful completion gates and fresh
fourth-request retirement. This is not a whole-interpreter stack bound or
sustained preemption/FP-pattern qualification.

The fresh kernel graph closes 67 register-call sites and seven indexed tables.
Bootstrap use is 44,672/65,536 bytes, leaving 20,864 bytes and 4,480 beyond the
separate required 16 KiB reserve. Including the 176-byte saved frame, the
SYSCALL/native/timer bounds are 5,296/5,184/3,296 of 16,384 bytes; the bounded
fault prefix uses 2,448 bytes. Generic fatal recursion, serial-format vtable
recursion and NMI nesting remain outside the certificate. Further bootstrap
observation growth needs renewed review of this measured remaining reserve.

Ten inspected shared entry/TLS paths normalize exactly to the preceding access
checkpoint. Emitted inspection confirms three separate reply/journal snapshots,
the volatile active-journal reset before caller selection, readback before a
fresh raw request, and final retained-journal clearing. The 4096-byte copy
scratch stays in retained BSS. TLS/FP ordering, terminal restoration and the
read-only embedded native blob bindings are covered by the frozen certificate.

Retained focused artifact SHA-256:

- Kernel ELF: `e309f10801554f4398019dcece910f7b27785f981e269d3e682c6851fd0a2da1`.
- Disk image: `590312600426ea64310804d615de6f489d947a6ae030304c22839c19ef45885f`.
- Native veil ELF: `143c5c924c2cf71ad0ce4fd8b8ffacf08fff24d95f7870f1e5a4e4f98e4e03c5`.
- Policy: `dc0039b851f0b9578f5d3a57036f92cdad8a603217396b5066f901e466e5054e`.
- Native plan: `b971aaf469ff272c81d3fba60e375d8f8d0059d7c46685b7b294ee0e58d5aee1`.
- Source snapshot: `b79d9ccc61e497bd773d199f430d4522278817b82e2d45de60d66219a345af90`.

The original main/interpreter files, source snapshot and native plan remain
unchanged from the preceding access checkpoint.

After the host capture fix, all eighteen retained compiled/package artifacts
from focused v2 are byte-identical to reviewed v1, including the complete
kernel/image and native ELF/payload/data/policy. The independent equivalence
receipt binds both source freezes, the host fix and its explicit regression
pass, all eight fresh focused jobs and the original source/emitted certificates.
The same measured stack/control bounds therefore apply to the final artifacts.

Each TCG/KVM run at 96/320 MiB passed seventeen ordered records: eight boot events, ready,
four boundaries, three committed full-frame replies and complete retirement.
It binds both copied names separately to their request sequence/address,
declaration and retained initialized read-only/nonexecutable source span.
The fourth syscall number and arguments remain observational. Measured totals
are four raw calls, three returns, 59 native calls, 48 copy attempts, 36
successes and twelve rejections. All three tasks retire; copy scratch and retained
journals are clear, CPU/TLS/root ownership is restored, and all 223 owned frames
are returned (124 execution-data, 28 page-table and 71 source frames).

All four runs have identical boundary records:

| Sequence | Actual request | Instruction | Next RIP | Result | Returned to Linux |
| --- | --- | --- | --- | --- | --- |
| 1 | `brk(0)` | `0x600284c9` | `0x600284cb` | `0x40009000` | Yes |
| 2 | `access("/etc/ld.so.preload", 4)` | `0x60029489` | `0x6002948b` | `-ENOENT` | Yes |
| 3 | `openat` of the declared-absent cache | `0x60029743` | `0x60029745` | `-ENOENT` | Yes |
| 4 | `openat`, pathname not copied | `0x60029743` | `0x60029745` | Unsupported bits `-38` | No |

The second and third pathname pointers are `0x60033720` and `0x60032491`;
nineteen and seventeen authenticated copied bytes include their respective NULs.
The fourth request has dirfd `0xffffff9c`, pathname pointer `0x7fff0000eec0`,
flags `0x80000`, mode zero, unused arg4 `0x7fff0000f0b7` and arg5 zero.
Its RSP is `0x7fff0000ee40`, R11 `0x206` and FS base zero. The pathname
pointer is inside the owned writable/nonexecutable stack, 128 bytes above RSP
and 4,064 below the initially populated stack suffix. No initialized retained
source span covers it. Original zero bytes cannot reconstruct the mutated
stack, so this pathname and its length remain unknown.

Evidence is retained under
an external validation directory (not distributed).
Prior evidence under `linux-loader-access` remains unchanged. Old fifteen-record
logs copied into a focused build bundle are not new physical evidence; new
backend logs are retained separately. Generated artifacts stay outside Git.

Missing, short and single-byte-corrupt source provisioning each reached its
expected attributed failure under TCG at 96 MiB. Original pinned inputs remained
unchanged. These negative logs do not independently expose final physical
resource accounting, so they do not establish the successful run's cleanup
claim. The final image also booted in VirtualBox 7.2.6 at 320 MiB and refused
the existing `application_cpu_amd_error_pointers` capability gate. The harness
recorded poweroff and no qualified feature event; VirtualBox is unsupported
for this proof on the current host, not a semantic pass.

## Final evidence and memory

Implementation checkpoint: `7fe153f2fcc59de1797974d7b0437cbde3707284`
(`Answer the Linux loader cache probe with fresh caller authority`). The preceding
host capture repair is `fc764f18363c683c2273659b6a38b2be2c1202fc`.
Both were pushed to `origin/main` after qualification. This log and the readiness
updates form the following documentation checkpoint.

The final manifest selects 44 successful guarded jobs across six scopes,
including focused tests, the complete 29-target matrix, both physical backends,
negative provisioning and the final baseline rebuild/capture. All nine attempted
scopes completed with final memory samples and stopped monitors. The original
focused v1 run, failed qualification v1 and unsupported VirtualBox attempt
remain chronological evidence; none substitutes for final selected successes.

Normal, workspace-dev, owned-workspace, CPU-workspace and native-editor images
are byte-identical to their initial baselines. Independent comparison checks
complete disks, Threshold geometry/checksums and the final CPU kernel's
ELF-to-raw-to-disk reconstruction. All 482 final frozen source files remain
unchanged.

The independent `final-audit-v1/receipt.json` passed at 18:38:07 UTC. It binds
the private policy and deployed native bytes, four interpreter syscall sites,
three complete reply-frame pairs, both separately copied pathnames, rejected
inputs, compiled-artifact reviews, baseline images and serialized guarded
intervals. Receipt SHA-256:
`6812c2f36b330e7083ecc5d10b94f0868e2052528715d59dc1c5a0ba21e06b84`.
The `final-evidence.json` manifest SHA-256 is
`31ad8911bdd2fc1691cc775abf9caaee59d726b0694daf4cb5fd2e261b803882`.
The all-four-run next-service handoff is separately bound with JSON SHA-256
`cb2890f9af6c811a826febf441dfdda32ceee60eb6a44510b3d0e7268bc0773f`.

Builds and VMs ran serially with one worker under the unchanged 4 GiB soft /
6 GiB hard / 512 MiB scope-swap guard. Across all attempts, peak scope usage was
3,395,543,040 bytes; available host memory stayed at or above 16,618,762,240
bytes. Scope soft/hard-limit and OOM counters remained zero, with no pressure
abort or runner error. The host kernel OOM counter remained zero through the
audit. Accessible unit/kernel journals showed no qualifying intervention; the
oomd query returned no accessible records. Collected units and missing history
remain unavailable, not historical zero. One-second sampling cannot establish
continuous pressure observation or reconstruct unlogged final PSI details.
Host swap already existed. No OOM or guard abort was observed in the retained
work.

## Remaining application work

No successful open, descriptor or file contents are supplied by this increment.
General file operations, dependency mapping/protection, address-space growth,
TLS/libc startup and a Linux process exit remain open before the larger Electron
process/display environment. The next service must admit authenticated native
operation 18 for the fresh fourth request and copy its live pathname before
selecting namespace behavior. Its oracle must qualify mutable memory rather
than compare it with original source bytes. A successful open then needs an
explicit namespace binding to a pinned held file and a task-owned descriptor.

The main executable declares `libc.so.6` as its only `DT_NEEDED` name, but that
does not identify the observed fourth pathname. The inspected libc has four
load segments and 545 pages. Naively adding all of it would exceed this proof's
existing plan/source budgets; those are removable experiment bounds, not OS
path or image limits. A held file can first supply bounded reads without mapping
the whole dependency. Antimatter launch or rendering is not established.
