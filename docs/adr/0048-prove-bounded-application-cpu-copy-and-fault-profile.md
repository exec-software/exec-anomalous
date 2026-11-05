# ADR 0048: Prove a bounded application CPU, copy, and fault profile

- Status: Proposed
- Date: 2026-10-03
- Builds on: [ADR 0047](0047-prove-sustained-dispatch-over-retained-launches.md)
- Direction: [ADR 0045](0045-select-native-antimatter-ny-and-hag-direction.md)

## Context

Native Antimatter is the intended first actual application ABI test. Ny's
current Linux runtime contains SSE instructions and thread-local state; an
integer-only fixture cannot qualify that runtime. Its compiler currently
selects the host target and ordinary hosted startup/linking. A useful next
kernel gate must own the machine state it admits, move bytes across the task
boundary with checked authority, and contain user faults independently of
fixture instruction locations.

## Decision

Add a removable `application_profile_test` feature above the retained two-launch
owner. Keep earlier GPR-only proofs and normal/development behavior separate.
This is an internal, bounded profile with no stable wire ABI or arbitrary
executable loader.

### CPU state

Admit x87, MMX, SSE and SSE2 legacy state with eager FXSAVE64/FXRSTOR64 ownership.
Own aligned supervisor images for the original kernel state, two tasks, and a
clean handler state. Save the interrupted user's state before checking it;
restore clean state while the kernel handles the transition; restore the
selected task before every user return. Initial user images contain clean
registers, masked exceptions and admitted MXCSR bits. Users never supply a
restore image. Kernel Rust stays compiled for the existing soft-float target.

Admission checks CPU capabilities and owns CR0/CR4/EFER policy. OSXSAVE/AVX,
nonzero TLS bases, lazy FP switching and other extended CPU profiles are not
admitted. AMD/Hygon CPUs without `XSaveErPtr` are rejected because their legacy
error-pointer behavior cannot support this proof's exact restoration claim.
Clear EFER.FFXSR while using legacy saves so all XMM state is owned. Verify
restoration of architectural kernel state and original control registers, then
volatile-clear retained state storage. TCG uses an explicit Intel baseline;
KVM qualification uses the actual host profile rather than disguising its
vendor/capabilities.

### Checked user copy

Resolve the exact current `task_identity` through the retained committed launch.
Require live task/circle generations, running attached scheduler context, the
selected root/TSS stack and the active retained owner. Limit an operation to
4096 bytes; reject an empty operation explicitly.

Preflight checked arithmetic, the whole low-user range, every region and page,
permissions and retained bound leases. Read live page-table paths only through
known retained table storage, compare against expected entries (allowing only
admitted hardware accessed/dirty changes), and require leaves to resolve to
the owned region. Do not follow arbitrary page-table addresses or dereference
user virtual pointers.

Construct short-lived supervisor-alias spans within the owner borrow. Complete
all fallible checks before byte copying under the sole-BSP, IF-clear exclusion
contract. Every returned rejection leaves the destination unchanged. An
unexpected privileged hardware fault remains fatal; this is not rollback of
arbitrary machine faults. No allocator mutation or borrowed span crosses a
scheduling step.

### Faults

Classify selected CPL3 #DE, #UD, #GP and #PF using architectural error fields,
not a known instruction address or fixture arm. Prove frame/identity/stack
provenance separately. A faulting RIP or RSP may be invalid and need not satisfy
return admission; a surviving task's frame must satisfy it before return.
CPL0, nested faults, unsupported vectors, reserved-bit/unsupported page-fault
errors and corrupt ownership remain fatal.

With a runnable peer, use the existing atomic fault-and-dispatch transition.
For a sole current task, add a transition to faulted that retains its context,
clears current and consumes no dispatch/preemption sequence. Clear the common
exception latch and resume the retained bootstrap continuation before reclaiming
that task's abandoned exception stack or mappings.

## Validation contract

- Host tests cover CPU profile rejection, copy geometry and live page-path
  permissions, fault classification, last-current terminal transitions and
  rejection without mutation. The strict transcript oracle tests fields,
  ordering, malformed evidence and independent process completion.
- Four sequential task pairs exercise first faults #DE/#UD/#GP/#PF, 64 PIT
  preemptions per pair, distinct legacy FP/XMM/MXCSR state, checked copies,
  16 survivor ticks, then a last-task #UD and complete cleanup. Initial state
  and restoration must be observed, not inferred from an instruction count.
  A fetch at absent RIP `0x802000` and final UD2 with RSP zero prove actual
  nonreturnable fault frames. Only the exactly armed private terminal return
  clears saved IF, preventing a timer between deliberate stack invalidation
  and fault; this fixture control is not a general user preemption policy.
- `make application-profile-image` builds `build/application-profile-test.img`;
  `make application-profile-smoke` runs both RAM sizes.
- Run the new proof at 96 MiB and 320 MiB under TCG and KVM, with exact restored
  memory accounting. Preserve rejected transcripts for diagnosis.
- Run all repository-required regressions, normal-image exclusion and relevant
  development/earlier owned-dispatch proofs. Freeze source for the matrix;
  record source changes and rerun affected checks if a correction is needed.
- Use two build jobs, a 4 GiB soft/6 GiB hard scope limit, and a memory watcher
  that terminates only this work's child process group on new OOM or less than
  4 GiB available host memory. Preserve logs and image hashes.

## Recorded evidence

All 28 regression targets passed on 265 unchanged frozen non-documentation
files in 1,153.82 seconds of summed target time. The fresh full host check
passed in 227.05 seconds with 440 default kernel, 483 all-feature kernel,
449 application-profile kernel and 106 xtask tests, plus native inspections
and strict Clippy. An earlier full check rejected a redundant return wrapper
in the new host harness; that single-file correction preceded the fresh freeze
and complete rerun. No earlier partial matrix is counted.

TCG Intel-baseline and KVM actual-AMD-host proofs passed at 96/320 MiB, accepting
16 records each and restoring 24,146/81,490 free frames. Each boot completed
four scenarios, 256 preemptions, eight terminal faults, 20 successful copies
and 40 rejections. Feature evidence differs only by the admitted CPU vendor;
both backends used the same proof-image SHA-256:
`88274ac9716bc04291a198be2b4e3ae15050d42af165709414d59baa55513bb0`.
Normal and workspace images match their previous checkpoints byte for byte.

The final matrix peaked at 1.984 GiB under a 6 GiB hard cap, with host available
memory at least 21.600 GiB. The complete development/validation sequence peaked
at 4.002 GiB and recorded 501 soft-limit reclaim events in the earlier failed
check. No hard-limit, scope OOM/OOM-kill or host OOM-kill event occurred.
No new VirtualBox evidence is claimed. The
[Application Profile Log](../../notebook/Development/2026-10-03%20Application%20Profile%20Log.md)
records source freezes, failed development attempts, all target results,
backend differences, hashes and limits. This ADR remains Proposed.

## Remaining gates

This profile does not port the hosted Ny runtime, supply TLS/threads, establish
a complete untrusted-code fault policy, or run native Antimatter. The next
application gate is a qualified freestanding Ny object/startup/runtime contract
followed by copied surface/input/workspace services and native editor rendering.
HAG remains a separate device/compute qualification; software rendering may
precede it. Existing Rust remains authoritative.

## Architectural references

Intel's [FXSAVE/FXRSTOR specification](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2a-manual.pdf)
defines the aligned 512-byte image, 64-bit error-pointer format and MXCSR
constraints. AMD's [OSRR 56255](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/56255_OSRR.pdf)
defines `XSaveErPtr`; [Linux v6.12 AMD initialization](https://raw.githubusercontent.com/torvalds/linux/v6.12/arch/x86/kernel/cpu/amd.c)
marks CPUs lacking it with the legacy save/restore leak quirk. AMD's
[APM volume 5](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/26569.pdf)
describes FFXSR's privileged XMM omission. These constraints motivate explicit
capability rejection and complete control-state ownership instead of lazy FP.

SYSCALL and SYSENTER admission stays disabled: require EFER.SCE clear and,
when SEP is advertised, IA32_SYSENTER_CS's selector clear. FSGSBASE must also
be disabled. Unmasked x87/SSE faults (#MF/#XM), debug exceptions and all other
unselected classes remain outside this task-local containment proof.
