---
title: "Linux Loader Entry Log"
date: 2026-10-08
tags: [linux, elf, syscall, userspace, loader, ownership, qualification]
status: qualified
---

# Linux Loader Entry Log

The unchanged Linux interpreter now executes inside Anomalous and reaches its
first genuine SYSCALL. The target remains the existing Linux Antimatter/Electron
application; this increment does not reach the main entry of `true`, initialize
libc or complete a Linux process. [ADR 0079](../../docs/adr/0079-reach-an-unchanged-linux-interpreter-service-boundary.md)
defines this separate, default-off `linux_loader_entry_test` proof.
Implementation checkpoint: `7fcc52aec8ddb235fb7743eeb0fef666a10877b1`,
committed and pushed after qualification; its non-Markdown source matches the
480-file frozen inventory.

## Implemented boundary

The host ELF adapter plans the original main executable, interpreter and initial
Linux stack. A linked 592-byte native plan binds exact mappings, source spans,
executable intervals and entry facts. The kernel does not parse ELF or implement
Linux syscall policy. A read-only QEMU fw_cfg item supplies 290,504 copied bytes:
the complete unchanged main/interpreter files and the 352-byte occupied stack
suffix. Trusted provisioning has its own source origin; it cannot impersonate a
task uploader. FNV-1a-64 checks accidental corruption against linked metadata,
not authenticity. The retained host evidence additionally binds SHA-256 hashes.

The source is sealed in 71 allocator-owned pages after chunked copying and
readback. Its temporary privileged alias and scratch are withdrawn before
consumer preparation. The mixed execution group owns nine caller regions and
three regions for each native peer/veil, with separate roots and transition
stacks. The caller has 88 user pages, including a guarded 64 KiB stack. Live
ownership totals 195 data frames, 28 table frames and 22 leases, including the
source. All 223 frames are released after retirement. The 512 KiB source budget
and fixed proof geometry introduce no global filename or path-length ceiling.

Real SYSCALL entry captures the raw number and six arguments. Authenticated
checked copy corroborates the instruction's `0f 05` bytes; the host oracle also
checks those bytes against the retained original interpreter. A separate native
userspace veil receives the 80-byte invocation and selects `retire_caller` with
`-ENOSYS` result bits. The controller validates the sealed suspended caller,
retires caller and veil without returning to Linux, and runs the native peer to
its normal native exit. CPU/TLS controls and storage, scheduler state, mappings,
source and exact initial allocator accounting must all be restored.

## Observed execution

TCG with Nehalem and KVM with the host AMD CPU both passed at 96 and 320 MiB.
All four boots used the same retained image and produced exactly eleven ordered
records: eight boot records, initial process facts, the actual boundary, and
complete retirement. Each records one raw Linux syscall, five native calls and
three retired tasks. There is no screenshot or application-rendering claim.

| Observation | Value |
| --- | --- |
| Interpreter initial RIP | `0x60023340` |
| Initial RSP | `0x7fff0000fea0` |
| Main AT_ENTRY | `0x40002db0` |
| Main PHDR | `0x40000040` |
| Observed syscall | 12, `brk(0)` |
| Actual SYSCALL instruction | `0x600284c9` |
| Saved next RIP / hardware RCX | `0x600284cb` |
| Saved RSP | `0x7fff0000fda8` |
| Hardware R11 / user FS base | `0x246` / zero |
| Userspace retirement result bits | `18446744073709551578` (`-38`) |
| Returned to caller / Linux exit observed | false / false |

The other five raw argument registers are retained as observed, without treating
their incidental values as additional `brk` arguments. No syscall number was
assumed by the acceptance oracle. `-ENOSYS` is the veil's native retirement
decision, not a return value observed by resumed Linux code.

Three independent 96 MiB TCG negative boots used the same image. Missing input,
a one-byte-short input, and a one-byte-corrupted input produced respectively
`linux_source_missing`, `linux_source_size` and `linux_source_hash`, with exit 35,
eight boot records and one exact fatal record. None emitted a caller boundary.
These observations prove rejection; their serial output does not independently
prove the full cleanup accounting measured by the successful path.

VirtualBox 7.2.6 booted the same image at 320 MiB, refused
`application_cpu_amd_error_pointers` before userspace, and was powered off cleanly.
It is an observed capability refusal, not a successful loader execution. This
profile also requires QEMU's explicit source-provisioning interface; no
VirtualBox equivalent was added.

## Emitted review and retained failures

The final entered-path review binds the emitted ELF, every disassembled byte,
sixty register-call sites and six indexed branch sites. Bootstrap use is
37,424/65,536 bytes, leaving 28,112 bytes and preserving the separate 16 KiB
preparation reserve. Including the 176-byte hardware frame, the largest
SYSCALL/native/timer paths use 5,424/4,864/3,296 of 16,384 bytes. The native veil
uses 464 of 8,192 user-stack bytes. The unchanged interpreter has a guarded
64 KiB stack; no whole-interpreter static stack bound is claimed.

The emitted review verifies TLS capture before eager save, TLS staging before
FP restore, final null selectors before FS restoration, and CPU/bootstrap
restoration before TLS clearing and mapping/source teardown. Generic fatal
formatting recursion and NMI nesting are outside the entered-path bounds; NMI
remains masked in this proof.

Early focused lint failures exposed an unspecified pointer cast, the unused
failure-renderer import, and dormant helpers inherited from older profiles.
These were corrected without weakening freestanding warning checks. The first
TCG attempt then rejected `loader_initial_state`: `run` checked IF before the
existing scheduler restriction helper could clear it. The corrected order
establishes the interrupt exclusion interval before checking it or withdrawing
the framebuffer. The failed image, trace and earlier emitted review remain
retained; a fresh emitted review covers the corrected image.

## Qualification evidence

The final non-Markdown source freeze covers 480 files. Final ELF SHA-256:
`d63684f3fbec1a88c49f81d7cc0180ddc44494575cfa7f2d6470da8e04f1999e`.
Final image SHA-256:
`031bf3631138872575b3fe99e267d2ed8bb983d8ef2560dec65e927ea4da5d7c`.
Retained source SHA-256:
`b79d9ccc61e497bd773d199f430d4522278817b82e2d45de60d66219a345af90`.
Plan SHA-256:
`b971aaf469ff272c81d3fba60e375d8f8d0059d7c46685b7b294ee0e58d5aee1`.

The main file is 35,288 bytes, SHA-256
`913a39cd38f353497086bcf317b12f91f93c23b51869cea763b8340b4f84cfd3`;
the interpreter is 254,864 bytes, SHA-256
`bda779ea9e9ad234f60315477d7cfd0bdde861cd8ddd9df8cdaf62f18c4cec14`.
The local evidence root is
an external validation directory (not distributed).
It contains frozen sources, full attempted logs, retained input/image bundles,
source and emitted reviews, memory telemetry and VirtualBox cleanup receipts.
Generated binaries remain outside Git.

The focused 603 kernel tests, 336 host-tool tests, focused lint/build checks,
TCG/KVM execution and negative-input checks passed. All sixteen
repository-required targets and thirteen affected regressions passed; `make check`
also exercised kernel configurations with 643 tests. An additional fresh
`owned-workspace-cpu-smoke` passed its complete desktop/editor and resource
restoration checks. The final independent audit passed at 05:40:27 UTC,
binding 41 selected successful jobs, all 14 attempted scopes, eight artifact
sets and the unchanged 480-file source freeze. The failed lint/first-TCG attempts
and unsupported VirtualBox attempt remain in the evidence inventory.

Final manifest: `final-evidence.json`, SHA-256
`4eeb58a4c6b173783987087b0bdd62cc2f24d8bb7e002c27e201698f66a01e07`.
Final audit: `final-audit-v1/receipt.json`, SHA-256
`a5ae1efd3d7b18358cac83bdba0e5516f4e82746dde705705542e884d6f571c4`.
The audit independently reconstructed the retained ELF/plan/source/stack and
Threshold image, checked the actual SYSCALL location, all four physical traces,
negative mutations/refusals, baseline reviews and VirtualBox cleanup, and
recomputed sampled guard decisions and serialized job intervals.

The freshly rebuilt normal, workspace-dev and owned-workspace images are
byte-identical to their retained baselines. The native-editor image differs by
four panic-location bytes and eight derived checksum bytes; exact image
reconstruction accounts for every difference. The CPU-workspace baseline had
last been rebuilt at commit `8bc98bb`, before later mapping-plan changes. Its
fresh image differs by 165,306 bytes, including shifted addresses, relocation
cells and diagnostics. Independent emitted review reduced the code difference
to the previously reviewed resident-plan constructor and inlined insertion sort;
its frame remains 480 bytes and the former nested 48-byte sort helper disappears.
No current-slice behavior change was found in that profile. The fresh CPU image
also passed 21 keyboard, 465 pointer/Appearance, 26 graphics and 95 editor frames,
with controlled faults, watchdog and complete resource-restoring exits.

All builds and VMs ran serially with one build/test worker under the unchanged
memory guard: 4 GiB soft limit, 6 GiB hard limit and 512 MiB scope swap ceiling.
The main qualification scope peaked at 4,297,064,448 bytes; available host memory
stayed at or above 19,952,091,136 bytes. The soft-limit counter advanced through
reclaim/throttling, while hard-limit and OOM counters stayed zero. No memory
pressure abort occurred. Host swap already existed; no zero-swap claim is made.
The host kernel OOM counter remained zero through the audit. Accessible unit and
kernel journals showed no qualifying intervention; the oomd query returned no
accessible records. Missing history is unavailable, not proof of historical
zero. One-second samples cannot establish continuous pressure observation or
reconstruct unlogged final PSI details. The supported conclusion is no observed
OOM or guard abort across the retained work.

## Next execution gate

`brk(0)` is the first measured missing service. The next increment can provide
explicit initial-break metadata to the userspace veil, answer that query through
the existing authenticated reply path, and resume unchanged ld.so until its next
real boundary. It needs a fresh request sequence and exact caller-frame/RAX
commit, not heap growth or a new native operation. Changing the break requires
separately qualified address-space policy and mechanisms; growing it must not be reported as successful without owned
writable backing. Requesting the existing break can be a no-op.

Subsequent address-space and regular-file/descriptor behavior remains userspace
policy over qualified native mechanisms. Follow unchanged ld.so through
dependency mapping, protection and TLS startup to an actual `true` exit.
Thread/process, event, filesystem and desktop services remain further work for
Electron/Antimatter.
The unsupported retirement proof is useful progress toward that target, not
application launch or a claim that the remaining compatibility work is small.
