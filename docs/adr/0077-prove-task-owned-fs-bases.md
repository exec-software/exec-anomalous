# ADR 0077: Prove task-owned FS bases

- Status: Accepted bounded proof; TCG/KVM/VirtualBox and required/affected regressions qualified
- Date: 2026-10-07
- Builds on: ADRs 0047, 0048, 0073, 0075 and 0076

## Context

The selected application target is the unchanged Linux Antimatter/Electron
build. The nearer integration target is unchanged dynamic `true`, its
interpreter and libc. The Linux image/stack planner and owned source/mapping
mechanisms do not supply task-local CPU state. Existing native entry stubs
reload FS/GS selectors and existing application/Linux profiles require zero
bases. A retained Rust field alone cannot preserve a nonzero FS base across
those transitions.

## Decision

Add a default-off `owned_tls_test` native proof over ADR 0075's retained
nine-region pair. Each committed identity owns a full-width FS base, initially
zero. User GS and KERNEL_GS bases remain zero. CR4.FSGSBASE stays clear and is
checked at admission and every active boundary. No SWAPGS or user FSGSBASE
instruction is introduced. This GPR-only proof does not select the separate
application FP/SIMD profile. `owned_source_test` takes precedence over this
proof; existing higher-priority application and diagnostic profiles retain
their behavior.

A non-copyable session retains two identity-bound task records and sole-BSP
ownership through explicit cleanup. A supervisor-only 72-byte assembly capsule
carries one exact frame handoff. Its phases distinguish inactive, clean kernel,
staged return, active user, captured entry and failed state. It is resident in
every retained root. The normalized 176-byte scheduler frame does not change.
IRQ/NMI exclusion and the existing nonreentry latch remain required.

After GPRs are saved on a trusted transition stack, assembly observes user
FS/GS selectors and FS/GS/KERNEL_GS bases before any selector reload. It clears
all three bases before Rust. The runtime authenticates the exact current task,
frame, retained root and TSS stack before consuming the capsule once. The
observed user selectors must be zero, GS/KERNEL_GS must be zero, and FS must
match the retained current task. Unsupported selector/base changes fail closed.
Kernel-origin entries are never interpreted as user captures.

The kernel stages a selected task only after validating its complete return
frame and selecting its root/TSS. Assembly loads zero user selectors first,
then writes and reads back the selected FS base at the final return boundary.
No selector reload follows that write. Same-task gates, timer switches and the
fault-to-survivor return use the same ordering. Inactive hooks do nothing, so
higher-priority profiles do not acquire a TLS session accidentally.

Experimental native operations 12/13 set/get the authenticated current task's
base at exact proof call sites. A setter accepts zero or a lower canonical
48-bit address and rejects every other value without state or hardware changes.
An accepted value need not be mapped: the base is address metadata, not memory
access authority. Page permissions still govern dereferences. No TLS layout,
TCB, DTV, allocation policy, Linux operation number or errno contract is imposed
by this native mechanism.

Admission requires bootstrap FS/GS selectors exactly `0x10` and all three
bases zero. Both task records are retired while on a trusted transition stack.
Terminal assembly retains clean kernel state while restoring the bootstrap
continuation. Explicit `finish` there restores and verifies the admitted
selector/base state, clears the owner and capsule, and only then permits
mapping cleanup. Failure retains ownership or fails stop; there is no fallible
Drop release. Arbitrary original segment-state restoration is not claimed.

## Physical proof

The same two retained tasks use distinct full-width bases in their existing
private high stacks. Each exercises initial zero, two denied values, A-to-B
replacement, clearing to zero, and restoration to A through ten native calls.
Actual FS-relative loads/stores check distinct canaries and local progress;
privileged reads corroborate the private physical stack bytes, including the
small permitted instruction-interruption window.

The existing dispatch lifecycle supplies 64 preemptions, an exact task-0 UD2,
16 further survivor callbacks and peer exit. The proof counts 106 captured
entries and 106 staged returns, checks kernel bases on each accepted entry,
and verifies complete TLS and retained-resource cleanup. Pure hostile tests
cover wrong/stale identities, exact frame binding, replay/phase errors,
unsupported controls/selectors/bases, unchanged rejected operations and fresh
zero state after retirement. A fresh second physical dispatch lifetime and
userspace FSGSBASE instruction fault are not claimed by this slice.

## Qualification boundary

Require hostile host tests, freestanding lint and feature precedence checks;
inspect emitted entry/return ordering and stack bounds before VMs; then run
strict ordered serial oracles under TCG/KVM at 96/320 MiB and isolated VirtualBox
where supported. Run all sixteen repository-required checks plus affected
shared-entry, multi-region, source, application and editor regressions. Serialize
builds and VMs under the memory guard, preserve failed attempts and compare
saved image baselines. The dated notebook records the final artifact/source
hashes, emitted review and exact results.

This establishes a native FS ownership mechanism, not Linux `arch_prctl`,
TLS-enabled SYSCALL, libc startup or Electron compatibility. Later work must
compose TLS with the existing eager CPU owner and Linux entry path, actual
image/startup sources, dynamic mapping/protection, and Linux file/descriptor
services. ELF interpretation remains outside the kernel. Existing zero-TLS
application/Linux profiles keep their admission rules until that composition
is separately qualified.

## Architecture references

- [Intel Software Developer Manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html): long-mode segment bases and RDMSR/WRMSR semantics.
- [Intel FSGSBASE guidance](https://www.intel.com/content/www/us/en/developer/articles/technical/software-security-guidance/best-practices/guidance-enabling-fsgsbase.html): CR4 controls the userspace instruction mechanism.
- [Linux x86-64 FS/GS documentation](https://kernel.org/doc/html/v6.16/arch/x86/x86_64/fsgs.html): FS-relative TLS and base-setting mechanisms.
- [Linux v6.17 process_64.c](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/kernel/process_64.c): selector/base restoration ordering and legacy CPU differences.
