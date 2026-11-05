# ADR 0080: Resume an unchanged Linux interpreter after a userspace reply

- Status: Accepted for the bounded interpreter-reply proof
- Date: 2026-10-08
- Builds on: ADRs 0053, 0073, 0078 and 0079

## Context

ADR 0079 qualified unchanged interpreter execution through its first real
SYSCALL, followed by explicit unsupported retirement. The measured operation
was `brk(0)`. The target remains the existing Linux Antimatter/Electron build;
the nearer gate is unchanged dynamically linked `true` completing startup and
exiting. Reaching one syscall did not meet either target.

The existing native scheduler already supports an authenticated reply to a
blocked caller and a sealed waiting-veil token for its next invocation. Reuse
that mechanism to advance the real loader one service boundary without adding
Linux policy to the kernel or claiming heap allocation.

## Decision

Advance the default-off `linux_loader_entry_test` profile to a bounded two-call
proof. Preserve ADR 0079 as the record of the previous one-boundary result.
Keep the same pinned unchanged executable/interpreter, native mapping plan,
retained source ownership, initial Linux stack and three-task CPU/TLS envelope.
No new native operation, general loader API or stable binary format is added.

The host supplies a private 128-byte userspace policy record in the native data
prefix: role at offset 0, `ANLPOL01` at 8, little-endian version 1 at 16, size
128 at 24, and initial break at 32, followed by zero reserved bytes. The host
derives the break from its ELF image plan and binds the complete record to the
copied data and retained artifacts. The kernel writes only the existing role
word. This metadata does not grant address-space ownership or allocation rights.

The userspace veil validates the record and accepts exactly syscall 12 with
argument zero equal to zero as its first request. Unused syscall argument
registers need not be zero. It returns the declared current break using native
action `return_to_caller`, with no staged effect. For this fixed image the value
is `0x40009000`; the veil receives it from explicit launch data rather than a
second independent constant. The raw Linux query result is the current break,
not the libc wrapper's success code. No pages are allocated or permissions
changed, and the break itself does not change.

The kernel validates nonzero exact request correlation, bounded phase, native
action, zero stage token and reserved words. It independently authenticates the
sealed pending caller, current veil, task generations, roots, TSS stacks and
saved frames. It validates both retained frame pointers before preparing the
scheduler reply, revalidates the caller against the prepared authority and
checks the exact saved-context destination before the infallible commit.

Only the caller's saved RAX receives the userspace result. All other saved
registers and return fields remain bit-for-bit unchanged. The suspended veil's
gate receives native OK with zero value/auxiliary before its next wake. Clear
the copied request and active sequence, but retain the sealed waiting-veil
token. Select the caller's owned root/TSS and SYSCALL controls, stage its FS
return capsule, restore its eager CPU image, and return through the existing
IRET path. Linux syscall numbers and result interpretation remain userspace
policy; the kernel's progression checks concern native actions only.

The next actual caller SYSCALL receives a fresh nonzero sequence and wakes the
same veil at its completed gate. The veil checks the native OK result, retrieves
the second request and explicitly selects unsupported retirement with `-ENOSYS`
result bits. Do not return those bits to Linux. The second syscall number is
observed rather than assumed. Then retire all tasks, clear request/CPU/TLS
storage, release mappings and source, and restore exact initial frame accounting.

## Qualification and limits

Require focused host tests, hostile policy/completion-shape tests, source
review and emitted entry/return/stack review before booting. Require all
repository checks and affected regressions before final qualification. Serialize
builds and VMs under the unchanged memory guard.
Retain failed attempts and capability refusals alongside passing evidence.

The physical oracle requires exactly thirteen ordered records: eight boot
records, initial process facts, first boundary, committed reply, second boundary
and complete retirement. Both syscall sites must resolve to `0f 05` in retained
unchanged interpreter bytes. The first operation/result must match the declared
query policy. The reply record contains the complete 176-byte saved caller frame
before and after commit, encoded as fixed-width lowercase little-endian hex.
Independently decode it, correlate it with the first boundary and compare all
twenty-one non-RAX words. The second authenticated trap demonstrates that the
caller actually resumed. Require two raw calls, one caller return, seven native
calls, three retired tasks and full ownership restoration.

Exercise TCG and KVM at 96/320 MiB where supported and attempt VirtualBox under
the existing capability gates. The source provisioning interface remains
QEMU-specific. Neither a capability refusal nor a host harness exit proves
successful guest execution. There is no rendering or GUI claim.

Qualification passed all 29 required/affected targets and the independent final
evidence audit. TCG and KVM each passed at 96/320 MiB with identical artifacts;
the next observed request was `access("/etc/ld.so.preload", R_OK)`. All five
development baseline images remained byte-identical. VirtualBox's CPU-capability
refusal is recorded separately. The
[Linux Loader Reply Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Reply%20Log.md)
retains the exact hashes, review bounds, audit correction and memory evidence.

This increment does not implement heap growth, descriptors, file lookup, mapping
services, dependency/TLS initialization, libc completion, `true` exit or
Antimatter startup. A future change to the program break requires separately
qualified address-space policy and mechanisms; growth requires owned writable
backing. Requesting the existing break may be a no-op. Follow the next observed
loader boundary rather than synthesizing a successful unsupported service.

## References

- [Linux v6.17 program-break implementation](https://raw.githubusercontent.com/torvalds/linux/v6.17/mm/mmap.c).
- [glibc 2.42 Linux brk wrapper](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/sysdeps/unix/sysv/linux/brk.c).
