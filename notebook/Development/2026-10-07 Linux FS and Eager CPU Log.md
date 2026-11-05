---
title: "Linux FS and Eager CPU Log"
date: 2026-10-07
tags: [linux, tls, syscall, userspace, cpu, ownership, qualification]
status: qualified-bounded-proof
---

# Linux FS and Eager CPU Log

The selected target remains the unchanged Linux Antimatter/Electron build.
This slice composes task-owned FS bases with the eager x87/SSE2 owner and real
Linux SYSCALL entry under [ADR 0078](../../docs/adr/0078-compose-linux-fs-and-eager-cpu-ownership.md).
It does not execute unchanged `true`, its interpreter, libc or Antimatter.

## Implemented boundary

The default-off `linux_tls_test` uses a Linux caller, the existing native editor
service and a separate userspace Linux veil. Native operations 21/22 query or
stage the exact blocked caller's FS under sealed scheduler call authority.
Staging changes no caller state. Completion validates the retained caller and
prepares the scheduler reply before committing FS metadata; failed replies
preserve the proposal for retry. Veil loss discards it and contains its caller.
The proposal binds both task generations, a nonrepeating TLS owner nonce and
the complete scheduler owner/service/request/return correlation.

The old two-task session remains 96 bytes; the three-task session is 136 bytes.
The supervisor entry capsule remains 72 bytes, with a separate active-slot
count, and the normalized frame remains 176 bytes. The pending proposal is
120 padding-free bytes with explicit volatile clearing. GS/KERNEL_GS stay zero;
FSGSBASE and OSXSAVE remain off. ELF and Linux policy stay in userspace.

Linux policy handles ARCH_SET_FS/GET_FS. The four-level Linux SET limit is
`0x7ffffffff000`, including rejection of the final canonical page; GET retains
the native mechanism's wider lower-canonical observation range. This policy
distinction was corrected after checking Linux v6.17 `task_size_max`, before
any VM run. Host tests cover both sides of the boundary and unchanged rejected
state. The native mechanism's address range did not change.

The caller uses actual FS-relative memory at `0x800020` and `0x800040`, plus
set/get of unmapped full-width `0x7fff12345000` without dereferencing it.
It checks x87 state, MXCSR and both halves of XMM15 across real SYSCALL returns
and a dedicated asynchronous timer witness. Peers have measured zero FS and
initially clean independent eager images; this slice does not add distinct
live peer FP patterns. Timer IRQs resume the interrupted task; task switches
occur through the call/reply and cooperative paths.

| Scenario | Raw Linux calls | FS queries | FS stages | FS commits | Caller FP entries | Dedicated timer witnesses |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Normal editor trace | 83 | 11 | 5 | 5 | 83 | 1 |
| Producer fault | 24 | 11 | 5 | 5 | 25 | 1 |
| Veil fault after staged SET | 4 | 3 | 1 | 0 | 4 | 0 |

The last scenario retires its caller with FS still zero. All three scenarios
erase their three TLS records and proposal, restore CPU/bootstrap state and
release retained mappings. Nine TLS-profile task lifetimes follow four native
baseline task lifetimes. The oracle requires exactly 26 ordered records and
accounts every TLS capture/return against observed gates, syscalls, IRQs and
faults. Captures independently verify the 384×24 public reference rectangle
(9,216 pixels) in a complete 1024×768 PPM; this is a reference fixture, not the
Antimatter GUI.

The TLS caller's fourteen additional FP/FS instruction sites have a separate
host inspection profile with exact helper extents, encodings and data targets.
Legacy profiles retain their instruction restrictions. All twelve legacy
payload blobs are byte-identical to the retained pre-change build.

## Emitted review

Final Linux TLS kernel SHA-256:
`c98be9879b17950aecb64f8d6ff8be2f6e62930dd78076491765cbfb772a52c6`.
Final image SHA-256:
`589e9e6c871cbbb0055bda78f4c3fc79a134ea102c994274291e9991893206b4`.
The final freeze covers 474 non-Markdown files.

The emitted SYSCALL entry still saves untrusted RSP then immediately loads its
trusted stack. Capture precedes selector reload; clean integer-only kernel
entry precedes eager save. Final null selectors precede full-width FS restore
and readback. Terminal cleanup preserves clean bases until bootstrap finish,
and all retained words are cleared before mapping release.

The reviewed bootstrap path uses 36,240 of 65,536 bytes, leaving 29,296.
The deepest private gate path including its 176-byte frame uses 10,664 of
16,384, leaving 5,720. The old two-slot proof was independently rebuilt and
reviewed: its preparation checkpoint still uses 48,752 bytes, leaving 16,784
(400 above the 16 KiB preparation reserve), with deepest normal preparation
49,824. Rechecking indirect memcpy calls corrects the previous private-stack
accounting by 16 bytes in both old and new artifacts: gate/timer/fault are
3,328/3,392/3,264. This is an accounting correction, not increased stack usage.

## Qualification

Focused host suites passed: 588 kernel tests, 329 tooling tests after the
instruction-profile extension, and 16 policy tests after the Linux boundary
correction. Formatting, payload/kernel lint and focused feature precedence
checks passed. Independent source and emitted reviews found no blocking issue.
The exact final image passed QEMU TCG and actual KVM at 96/320 MiB, plus separate
host-quit captures. Both backends produced the same captured frame hash:
`72f805481723f5ef47763584a72b6fe1b069df19ab792a970c261b722d7b64fd`.

The complete sixteen required plus twelve affected regression targets passed
against the final frozen source. The serial jobs took 1,696.95 seconds in total.
The affected targets covered native TLS, multi-region and copied-source owners,
sustained dispatch, application CPU state, ELF/Linux application entry, the
single/shared veil, ELF relaunch, the native editor workspace and its owned
workspace image. The code checkpoint is `59ab9a0`; the final source freeze was
taken before that commit, and all 474 captured files match the committed tree.

VirtualBox 7.2.6 booted the exact final image at 320 MiB but refused
`application_cpu_amd_error_pointers` before userspace. The harness retained the
nine-record transcript and powered off the isolated VM. Its failed harness
result remains in the audit as an unsupported CPU capability; no Linux TLS
guest execution pass is claimed for VirtualBox.

Four retained image baselines are byte-identical: normal, workspace development,
owned workspace and owned workspace CPU. The native-editor image changes only
twenty bytes: eleven Rust panic-location line fields account for twelve bytes,
and two derived boot checksums account for eight. Adding twelve source lines
before those scheduler locations explains the shift. Independent reconstruction
reproduces the entire changed image; the captured matching ELF corroborates
unchanged instructions, payloads, writable data, entry and mapping geometry.
Its accepted image hash is
`7df1201eb10289e01462411af347d251359c365894af8e418fab8b8114544bf5`.

All builds/tests/VMs run serially under the unchanged guard: one Cargo job,
one test thread, MemoryHigh 4 GiB, MemoryMax 6 GiB, swap cap 512 MiB, a 30-second
quiet admission period, one-second pressure sampling and a 4 GiB available
memory floor. Developmental failures are retained: a test binding collided
with a phase constant, selected-out helpers needed exact feature gating, and
the legacy instruction inspector initially refused the intentional FP/FS
fixture. Across all retained attempts, peak scope memory was 4,297,056,256 bytes
(4.002 GiB), and minimum host available memory was 9,460,166,656 bytes
(8.810 GiB). The final 28-target run retained at least 20,089,024,512 available
bytes (18.709 GiB). No guarded OOM, hard-limit event or pressure abort was
observed; the host OOM-kill counter remained zero. Collected systemd units and
inaccessible journal data are recorded as unavailable, not historical zeros.
One-second sampling does not claim continuous observation of brief pressure.

The final evidence audit passed. It binds 37 selected successful jobs, seven
artifact sets, the exact TCG/KVM captures, the separate VirtualBox refusal,
independent reviews and all earlier failed attempts. No unexplained baseline
image change remains. The manifest SHA-256 is
`1760ad2c7b104f037b665595b4f7b1df499d83bfdaed5f4e349635aa8b3aef5b`;
the `final-audit-v1/receipt.json` SHA-256 is
`bacf26b38dff6a0e696615507461e54b272aba9f87ced01e745f9dba8c24b66b`.
Final documentation changes do not alter the qualified source or artifacts.

Host evidence is retained at
an external validation directory (not distributed), including all guard
attempts, source freezes, exact artifacts, source/emitted reviews, backend
invocations and transcripts. Generated artifacts remain outside Git.

## Remaining execution work

The next measurable step is entering the unchanged `true` interpreter on its
real mappings and Linux initial stack, delivering its first real SYSCALL to
the supervised veil, and reporting the next unsupported service explicitly.
That boundary is not successful completion of `true`.

The inspected main/interpreter layout fits nine regions and 88 user pages,
including its 64 KiB stack. Their 290,152 source bytes do not fit the current
kernel's remaining boot staging space. A separate bounded read-only source
transport is therefore an explicit prerequisite; the five-sector durable test
device is not already a general loader source. Mixed-role source admission,
real startup facts and checked suspended copies must join the existing owners
without retaining fixture-only FS/FP assumptions or putting large buffers on
the bootstrap stack.

Dynamic mapping/protection and Linux regular-file/descriptor services remain
gates before unchanged dynamic `true` completes with libc. Clone/threads,
signals, user GS, XSAVE and SMP are outside this proof. The larger process,
file, display and input environment needed by Antimatter/Electron follows the
small unchanged executable. The evidence folder's `next-linux-execution.md`
records the inspected source hashes, integration seams and proposed acceptance.
