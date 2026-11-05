---
title: "Linux Loader Reply Log"
date: 2026-10-08
tags: [linux, elf, syscall, userspace, loader, reply, ownership, qualification]
status: bounded-proof-qualified
---

# Linux Loader Reply Log

The unchanged Linux interpreter now receives one userspace policy reply,
resumes, and reaches its next actual SYSCALL inside Anomalous. The existing
Linux Antimatter/Electron application remains the target. This increment does
not reach `true`'s main entry, initialize libc or complete a Linux process.
[ADR 0080](../../docs/adr/0080-resume-an-unchanged-linux-interpreter-after-a-userspace-reply.md)
advances the default-off loader-entry profile from the preceding
[one-boundary proof](2026-10-08%20Linux%20Loader%20Entry%20Log.md).
Implementation checkpoint: `ff97a1f3ff4f0521c3540e581a62347315db10ba`,
committed and pushed after qualification; its non-Markdown source matches the
481-file frozen inventory.

## Implemented reply

The host supplies a private 128-byte policy record in the native peer/veil data
prefix. `ANLPOL01`, version 1, exact size, role, initial break and zero reserved
bytes are validated and bound to the retained native data. The initial break is
derived from the host ELF plan, not a second constant in the veil. The kernel
writes only the existing task role word. The record is inert userspace policy
metadata, not mapping or allocation authority.

The veil accepts the first measured `brk(0)` query and returns `0x40009000`
through native action zero without a staged effect. Unused syscall argument
registers are not required to be zero. No pages are allocated and the break
does not change. Kernel checks concern exact request correlation, native action
phase, reserved arguments, sealed task/scheduler authority and retained frames;
the kernel does not interpret the Linux syscall number or choose its result.

All fallible admission checks precede the prepared scheduler reply commit. Only
caller RAX changes; the complete 176-byte caller frame is retained before and
after for independent comparison. The veil's suspended gate receives native OK
with zero value/auxiliary. The copied request and active sequence are cleared,
while the sealed waiting-veil token remains available for the next handoff.
The caller's root/TSS/SYSCALL controls are selected, its FS capsule staged, and
its eager CPU image restored before the existing IRET path returns to Linux.

At the second actual SYSCALL, a fresh sequence wakes the same veil at its saved
gate. It checks the OK completion, retrieves the new request and chooses native
unsupported retirement with `-ENOSYS` bits. Those bits are never returned to
Linux. The native peer completes, all three tasks retire, and exact source,
mapping, scheduler, CPU/TLS and frame-accounting restoration is required.

## Physical observations

TCG/Nehalem and KVM/host passed at both 96 and 320 MiB. All four runs used the
same retained image, source, native plan and policy. Thirteen ordered records
contain eight boot events, initial process facts, the first boundary, committed
reply, second boundary and final restoration. Each run records two raw calls,
one caller return, seven native calls and three retired tasks.

| Observation | First boundary | Second boundary |
| --- | --- | --- |
| Wire sequence | 1 | 2 |
| Linux syscall | 12, `brk(0)` | 21, `access(path, R_OK)` |
| Actual SYSCALL instruction | `0x600284c9` | `0x60029489` |
| Saved next RIP / RCX | `0x600284cb` | `0x6002948b` |
| Saved RSP | `0x7fff0000fda8` | `0x7fff0000fae8` |
| R11 / user FS base | `0x246` / zero | `0x246` / zero |
| Userspace result bits | `0x40009000` | `18446744073709551578` (`-38`) |
| Returned to caller | true | false |

Both reported instruction sites resolve independently to `0f 05` in the exact
retained interpreter. The reply's two fixed-width lowercase hex records decode
to twenty-two little-endian frame words. The host checks all twenty-one non-RAX
words unchanged and correlates the original registers/return frame with the
first boundary. The second authenticated trap proves actual resumed execution;
the report alone does not assume it from a committed scheduler transition.

The second call has pathname pointer `0x60033720` and mode 4. The exact nineteen
bytes including the terminating NUL are `/etc/ld.so.preload`, within the
interpreter's initialized read-only LOAD. They occupy file offsets
`0x33720..0x33733` and source-snapshot offsets `0x3c0f8..0x3c10b`.
Local Linux headers identify syscall 21 as `access` and mode 4 as `R_OK`;
glibc's loader source confirms the optional preload-file probe. The current
veil does not service this pathname or claim a guest filesystem.

Three negative 96 MiB TCG boots rejected missing, one-byte-short and corrupted
source inputs with `linux_source_missing`, `linux_source_size` and
`linux_source_hash`. Each produced eight boot records plus the exact fatal
record, exit 35 and no caller boundary. This proves rejection, not independently
measured cleanup accounting for those negative paths.

VirtualBox 7.2.6 booted the same image at 320 MiB, refused
`application_cpu_amd_error_pointers` before userspace, and powered off cleanly.
This is a capability refusal, not a guest execution pass. The source input also
still depends on the QEMU-specific provisioning interface.

## Review and evidence

Independent source review covered reply preparation, exact caller/veil frame
admission, waiting-authority reuse, first-result publication, second correlation
and terminal ownership restoration. The fresh emitted review closed sixty-eight
register-call sites and six indexed tables. Bootstrap use is 38,416/65,536 bytes,
leaving 27,120 bytes and preserving 10,736 bytes beyond the separate 16 KiB
reserve. Including the 176-byte saved frame, SYSCALL/native/timer paths use
5,296/5,024/3,296 of 16,384 bytes. The bounded fault prefix uses 2,448 bytes;
generic fatal recursion, serial-format vtable recursion and NMI nesting remain
outside this certificate.

The native veil's maximum reviewed stack path is 776/8,192 bytes, including its
failure path. Emitted inspection confirms exact policy loads, caller RAX-only
store, saved veil OK writes, TLS capture before eager save, TLS staging before
FP restore and terminal ownership teardown ordering. No whole-interpreter
static stack bound or sustained preemption/FP-pattern proof is claimed here.

The non-Markdown source freeze covers 481 files. Retained artifact SHA-256:

- Kernel ELF: `e2b83d5f3cdae1f68a4fc4da9da8199ee4238ddfcd550a45cd813d378441cf15`.
- Disk image: `daf783d621617f0ca359bc0d79e0c79eb175e8de40f3e4c95ac8a399689334c7`.
- Source snapshot: `b79d9ccc61e497bd773d199f430d4522278817b82e2d45de60d66219a345af90`.
- Native plan: `b971aaf469ff272c81d3fba60e375d8f8d0059d7c46685b7b294ee0e58d5aee1`.
- Userspace policy: `85a0eb5818aded78a6dbe73aee512d93d5972ff200ba2d822313f998304cafd8`.

The original main/interpreter files, source snapshot and native plan match the
preceding loader-entry proof. The source snapshot still occupies 71 pages;
the three-task execution owner still accounts for 124 data and 28 table frames.
All 223 owned frames, including the source, are restored after retirement.

Local evidence is retained under
an external validation directory (not distributed).
It includes source/emitted reviews, all attempted logs, frozen inputs, separate
TCG/KVM bundles, negative provisioning receipts and VirtualBox cleanup. Old
eleven-record logs copied into the focused build bundle are explicitly excluded
from this increment's physical evidence. Generated artifacts are outside Git.

Focused validation passed 608 kernel tests, 343 host-tool tests, freestanding
kernel/host warning checks and the image build. All twenty-nine regression
targets passed: the sixteen repository-required checks and thirteen affected
execution, veil, ELF, TLS and workspace checks. The final manifest selects 42
successful guarded jobs across six scopes, including artifact captures and the
final CPU-workspace image rebuild. The separately attempted VirtualBox refusal
is not a selected success. Normal, workspace-dev, owned-workspace,
CPU-workspace and native-editor images are byte-identical to their initial
baselines; independent comparison also checked complete kernel payloads and
Threshold geometry/checksums. All 481 frozen source files remained unchanged.

The first evidence audit stopped at an orchestration-label mismatch: it expected
`tcg` for the Python negative-test runner, while the guard correctly recorded its
default `host` label. The independently reviewed adapter correction requires
that exact host label and preserves separate TCG checks on the runner receipt
and every absolute QEMU child invocation. No test, artifact, guard or repository
source changed. The original audit script and failure receipt remain retained.
The corrected `final-audit-v2/receipt.json` passed at 15:49:31 UTC, independently
binding policy/ELF/image bytes, both syscall sites, every reply-frame word,
negative inputs, baseline images and serialized guarded intervals. Its SHA-256
is `7ae3618b761831d9242caa9ef0f358cf8c68e4a6a34fe3daf1b069ce0183518b`;
the selected `final-evidence-v2.json` manifest SHA-256 is
`19cb2b44512d51b38454f0f626cb4c00ce16caf6e78b6a300e1bf4fbf6e2814f`.

Builds and VMs ran serially with one worker under the unchanged 4 GiB soft /
6 GiB hard / 512 MiB swap memory guard. Peak scope usage was 4,296,925,184 bytes;
available host memory stayed at or above 15,085,613,056 bytes. The soft-limit
counter advanced during reclaim, while hard-limit and scope OOM counters
remained zero. No pressure abort occurred. The host kernel OOM counter remained
zero through the audit. Accessible unit/kernel journals showed no qualifying
intervention; the oomd query returned no accessible records. Collected units and
missing history remain unavailable, not historical zero. One-second samples
cannot establish continuous pressure observation or reconstruct unlogged final
PSI details. Host swap already existed. The supported conclusion is no observed
OOM or guard abort across the retained work.

## Next service boundary

The measured next request is `access("/etc/ld.so.preload", R_OK)`. The next
bounded increment should expose the existing native copied-caller read under
the exact sealed pending authority and give the veil an explicit initial
namespace view. An absent optional file can then produce truthful `-ENOENT`
from userspace policy. Do not synthesize success, infer namespace absence from
the host, or claim general filesystem/descriptor support.

Read caller memory in checked chunks, stopping at NUL without requiring a
following page to be mapped. Per-operation byte/work budgets must not become a
global pathname limit. After a second authenticated reply, observe the third
real loader request before choosing further services. File descriptors,
dependency mapping/protection, TLS/libc startup and the much larger Electron
process/display environment remain open.
