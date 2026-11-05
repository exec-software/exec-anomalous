# ADR 0078: Compose Linux FS and eager CPU ownership

- Status: Accepted for the bounded supervised Linux FS/eager-CPU composition
- Date: 2026-10-07
- Builds on: ADRs 0048, 0053, 0073 and 0077

## Context

The existing Linux Antimatter/Electron build remains the application target.
Unchanged dynamic `true`, its interpreter and libc are the nearer execution
target. ADR 0077 proves native task-owned FS bases, but the application profile
also owns x87/SSE2 state and Linux enters through SYSCALL. These mechanisms
must work together before libc can initialize and use thread-local storage.

## Decision

Add a default-off `linux_tls_test` profile to the existing three-task userspace
veil composition: one Linux caller, one native editor service and one Linux
compatibility service entered through the native ABI. The profile takes
precedence over `shared_veil_test` within that application family. Existing
higher-priority diagnostics and application profiles keep their precedence.
The original zero-TLS profiles retain their admission requirements and payloads.

Compose the task-owned FS session with the existing eager x87/SSE2 owner. A
fixed three-record session remains in supervisor storage; the two-record
native session and 176-byte normalized frame retain their existing layouts.
User GS and KERNEL_GS remain zero, and FSGSBASE and OSXSAVE remain disabled.
The existing sole-BSP, interrupt exclusion and nonreentry requirements apply.

On every active entry, assembly captures user segment state after saving GPRs
on a trusted stack and before reloading selectors. It clears bases before
Rust. The SYSCALL prefix still saves untrusted RSP and immediately selects the
trusted transition stack. Rust authenticates the exact task, frame, retained
root and TSS before consuming the TLS capture and switching to clean kernel
floating-point state. Return selects and validates the destination, stages
its FS value, and restores its eager CPU image. Assembly loads final null
selectors before restoring FS and performs no later selector reload.

Linux policy remains in userspace. A separate opt-in policy extension handles
`arch_prctl` number 158, accepting `ARCH_SET_FS` and `ARCH_GET_FS`. The option
uses the Linux signed 32-bit argument interpretation. SET accepts zero and
bases below the four-level Linux user limit, `2^47 - 4096`, returning EPERM for
other values, including the final canonical page. The native mechanism still
accepts the entire lower canonical range; this narrower limit is Linux policy.
A base need not be mapped; it conveys no memory access authority. GET copies exactly eight
little-endian bytes through the existing checked caller-copy mechanism and
returns EFAULT when the destination is not writable. Unsupported GS and
unknown options return EINVAL. This is a bounded option subset, not the full
Linux arch_prctl interface.

Two experimental native service operations expose mechanism without Linux
policy: operation 21 queries the exact suspended caller's FS; operation 22
stages a proposed FS update. Both carry the current request sequence and no
caller identity, root, frame or hardware address authority. Reserved words
must be zero. The existing 80-byte request and reply wire formats do not change.

The sealed scheduler call authorization identifies the blocked caller. Every
query, stage and completion validates that authorization and the retained
caller frame, while requiring the current task to be the authorized veil.
A move-only staged effect binds the TLS session's nonrepeating owner nonce,
both task generations, scheduler owner/service/request/return correlation and
the caller's prior FS value. It cannot be redirected to another call or a
fresh session with reused numeric task IDs. Staging changes neither retained
caller FS nor hardware FS and excludes simultaneous memory/channel effects.

Completion first validates the caller frame and prepares the scheduler reply.
Only then does it commit the retained FS metadata and perform the existing
infallible prepared scheduler commit. Hardware FS changes only on a selected
return. A rejected completion preserves the staged effect for an exact retry.
A faulted veil discards its proposal and contains the dependent caller. The
proposal uses explicit all-word storage and in-place clearing, avoiding
unobservable inactive enum payload bytes during retirement.

Tasks lose TLS ownership before mappings can be reclaimed. Terminal assembly
returns with clean kernel bases to the bootstrap continuation; explicit TLS
finish there verifies the admitted bootstrap state and clears the session and
handoff capsule before retained execution resources are released.

## Qualification boundary

The bounded Linux fixture exercises mapped FS-relative canaries, A/B/zero
replacement, a full-width unmapped base, denied bases and invalid GET outputs.
Real SYSCALL boundaries, timer entry/return, cooperative task switches, caller
faults and a fault after a
staged SET must preserve the caller's x87/XMM15/MXCSR witnesses and the other
tasks' zero FS. All three eager images start clean; distinct live peer FP
patterns remain covered by the existing CPU ownership regression rather than
this fixture. The staged-fault scenario must prove no caller FS update committed.

Qualification requires hostile policy/ownership/oracle tests, freestanding
lint and feature precedence, emitted stack and entry/return inspection, strict
ordered transcripts under TCG/KVM at 96/320 MiB, and isolated VirtualBox where
the required application CPU profile is supported. A capability refusal is
reported separately and never counted as a successful guest proof. Serialize
builds and VMs under the unchanged memory guard, retain failed attempts, run
all repository-required and affected regressions, and compare image baselines.

The bounded implementation is qualified: final TCG/KVM runs passed at 96/320 MiB,
with 26-record transcripts and captures whose oracle verifies the public
384×24 fixture region (9,216 pixels), not whole-desktop semantics. All sixteen
required checks and twelve affected regressions passed. VirtualBox booted but
refused `application_cpu_amd_error_pointers` before userspace; cleanup powered
off the VM. This is an observed capability refusal, not a Linux TLS execution
pass. Exact source/artifact bindings, corrected attempts, emitted reviews and
memory evidence are retained in the
[Linux FS and Eager CPU Log](../../notebook/Development/2026-10-07%20Linux%20FS%20and%20Eager%20CPU%20Log.md).

This proof does not execute unchanged dynamic `true`, libc or Electron. It
does not provide clone/thread creation, signal delivery, user GS, FSGSBASE,
XSAVE, SMP, dynamic memory mapping or Linux descriptor services. Those remain
separate work needed to launch the existing Linux Antimatter application.

The next admission step connects separately provisioned source snapshots and
real image/startup plans to this composition, executing unchanged ld.so until
an attributable loader-service boundary. Reaching that boundary must not be
reported as successful completion of `true`.

## Architecture references

- [Linux v6.17 process_64.c](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/kernel/process_64.c): FS operation behavior, invalid-base errno and eight-byte GET output.
- [Linux v6.17 arch_prctl constants](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/include/uapi/asm/prctl.h): operation numbers.
- [Linux v6.17 task_size_max](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/include/asm/page_64.h): the final canonical page is outside the four-level Linux user range.
- [glibc 2.42 x86-64 TLS initialization](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/sysdeps/x86_64/nptl/tls.h): initializing the thread pointer through ARCH_SET_FS.
- [Intel Software Developer Manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html): segment bases, privilege transitions and eager floating-point state instructions.
