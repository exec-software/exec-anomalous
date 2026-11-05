# ADR 0079: Reach an unchanged Linux interpreter service boundary

- Status: Accepted for the bounded interpreter-entry proof
- Date: 2026-10-08
- Builds on: ADRs 0048, 0053, 0073, 0074, 0075, 0076, 0077 and 0078

## Context

The application target remains the existing Linux Antimatter/Electron build.
The nearer target is unchanged dynamically linked `true`, its interpreter and
libc. The preceding slices establish image/stack planning, owned multi-region
execution, retained source snapshots and supervised FS/eager-CPU ownership.
They do not establish execution of ordinary Linux binaries.

Connect those mechanisms without moving ELF parsing or Linux syscall policy
into the kernel. The first acceptance boundary is actual interpreter execution
through its first SYSCALL and an attributable userspace service decision. That
boundary is useful even while the file, mapping and process services required
for completion remain absent. It must not be reported as `true` exiting.

## Decision

Add a default-off `linux_loader_entry_test` profile, depending on the qualified
Linux TLS and owned-source mechanisms. Select a separate minimal three-task
runtime within the application proof family: one Linux caller containing main,
interpreter and stack, one native peer, and one native userspace veil. Preserve
the precedence of earlier high-priority diagnostics and the existing profiles
when this feature is absent. The new runtime does not reuse the editor's
application channels, rendering fixture or scripted Linux caller.

The host reads and retains the exact installed `/usr/bin/true` and explicitly
supplied `/lib64/ld-linux-x86-64.so.2` bytes. This proof pins their reviewed
SHA-256 hashes and rejects replacements pending explicit review. The portable
ADR 0074 parser plans the two images outside the kernel. It requires this
profile's four LOADs per image, supported permissions and metadata, exact
initialized executable intervals and disjoint address geometry. The original
files are neither patched nor executed by the host builder. Host path lookup
does not establish a guest namespace or authenticate an interpreter.

Package only the small native peer/veil payload and an inert native plan into
the kernel image. Supply the larger original files and used initial-stack bytes
through a separate retained source snapshot. Native population copies exact
source spans and verifies initialized bytes, BSS, holes and page slack. The
interpreter runs its own code; the kernel does not perform ELF relocation,
library discovery, GNU-property interpretation or Linux loader policy.

## Provisioned bytes and native admission

Add a distinct privileged provisioning origin to `owned_source`. It is not a
fabricated uploader identity. Task append/seal reject provisioned owners, and
controller append/seal reject task owners. Numeric grants, source hashes and
source contents confer no mapping, task or executable authority.

The selected QEMU boot controller reads only
`opt/anomalous-linux-source` through the traditional read-only fw_cfg interface.
It validates the signature, interface presence, bounded directory, unique exact
name, read selector, reserved fields and exact linked expected length. The
operation admits at most 512 KiB, 128 directory entries and 4096 bytes per
chunk. Those are explicit proof resource budgets, not global path, name, file
or executable size limits. This is not a new filesystem or general loader API.

The controller retains an allocator-owned, zeroed extent and an independent
temporary physical alias. Sequential copied chunks undergo complete readback;
the cursor advances only after success. Exact length and FNV-1a-64 must match
trusted linked metadata before sealing. FNV is accidental-corruption and
evidence bookkeeping, not cryptographic authentication. The host also retains
SHA-256 evidence for the exact files, plan, source and random input; trust still
comes from the explicitly selected provisioning controller and reviewed build.

The linked experimental plan is 592 bytes of explicit little-endian words:
sixteen header words, nine region triples, nine source-span triples and two
executable intervals. It records native mapping/entry facts and the expected
source length/hash, not ELF headers or a task-selected permission grant. Main
entry/PHDR, interpreter bias and proposed break are diagnostic facts; the last
does not implement `brk`. This private encoding is not a stable launch ABI.
The decoder revalidates native budgets, bounds, W^X, initialized executable
coverage and guarded-stack admission before allocation.

The trusted mixed profile admits exactly nine caller regions and ordinary
three-region native peer/veil mappings. The caller has 88 user pages including
a guarded 64 KiB stack; each native task has eight RX, two RW/NX data and two
stack pages. Each task retains its own root and 16 KiB supervisor transition
stack. The execution group accounts for 124 data frames, 28 table frames and
21 leases, plus the separately owned source extent and its lease. Copy and
suspended-caller checks select the nine/three policy from retained launch
provenance, never from an observed mapping count. Exact task generations,
scheduler authorization, root, TSS stack and saved frame remain mandatory.

## Initial process and CPU envelope

Use main bias `0x40000000`, interpreter bias `0x60000000` and stack base
`0x7fff00000000` as fixed proof choices. Initial RIP is the interpreter entry;
AT_ENTRY is the main entry. RSP addresses the 16-byte-aligned Linux initial
stack with no return slot; RDX and RBP start at zero and RFLAGS at `0x202`.
This is not ASLR or a general address-allocation policy.

The stack supplies `argv = ["/usr/bin/true"]`, an empty environment, main
PHDR/PHENT/PHNUM, page size 4096, main entry, interpreter base, exec filename,
sixteen retained random bytes and terminating null entries. UID/EUID/GID/EGID
are explicitly 1000, secure mode is zero, and CLKTCK is 100. These are selected
Linux-personality facts, not inferred host credentials or working clock syscalls.
Random bytes come from an explicit sixteen-byte input or host randomness retained
for reproducible rebuilding; no guest entropy service is claimed.

HWCAP and HWCAP2 are explicitly zero, and no platform pointer or vDSO is supplied.
These conservative auxv values do not mask CPUID. The QEMU TCG proof selects the
Nehalem CPU model, while the KVM qualification selects `host`; both must pass
actual CPU admission. The kernel admits
only its existing eager x87/SSE2 and task-owned FS envelope: user GS/KERNEL_GS
remain zero; five-level paging, FSGSBASE, OSXSAVE, protection keys and CET controls
must be disabled. No AVX/XSAVE, CET, general hardware-capability compatibility,
libc TLS initialization or new FS-setting syscall is proved by this increment.

Reuse the owned SYSCALL entry, eager CPU images and three-task FS session from
ADR 0078. Entry authenticates the retained context before consuming its capture;
Rust runs with clean kernel segment/FP state. Selected returns restore the exact
task's state. Timer entries may occur and use the same capture/return checks;
this short boundary proof is not the sustained-preemption or distinct live
FP-pattern proof, which remains covered by earlier native regressions.

## Userspace boundary and retirement

On the first real SYSCALL, verify the hardware frame relationships and copy back
the preceding `0f 05` instruction from admitted caller memory. Forward the raw
number and six arguments through the unchanged 80-byte invocation to the native
veil under sealed scheduler call authority. The kernel does not choose a Linux
result from the syscall number or synthesize a fixture call.

The minimal userspace veil retrieves that request and explicitly chooses the
existing native `retire_caller` completion with result bits for `-ENOSYS`.
The runtime authenticates completion, validates the suspended caller and consumes
the prepared scheduler reply. It retires the veil and caller without selecting
the caller's hardware context or returning to its next instruction. The native
peer then starts and exits through the native ABI. The result is a reported
unsupported service boundary, not an errno observed by resumed Linux code,
successful Linux `exit`/`exit_group`, execution of the main entry, or completion
of `true`. No particular first syscall number is assumed for acceptance.

The source controller remains in retained supervisor storage throughout the
consumer lifetime. It exposes a reader only after scratch clearing and temporary
alias removal. A failed import cannot resume its firmware stream; cleanup retains
partial source/alias/scratch ownership until clearing succeeds. Failed source
clear keeps the exact unbound lease for retry. No fallible Drop abandons ownership.
Privileged hardware faults or broken root invariants fail closed with retained
owners; retryable clearing is not a promise of arbitrary fault recovery.

Retire scheduler/TLS authority before reclaiming mappings. Restore the bootstrap
continuation, CPU/MSR/selector controls and cleared TLS/CPU storage, withdraw task
contexts, and finish the execution owner before reacquiring the source cleanup
alias. Clear/release the source and remove that alias last. Exact initial frame
accounting must be restored. The existing sole-BSP, IF-clear/NMI-masked,
nonreentrant ownership contract and stack/headroom limits remain unchanged.

## Qualification boundary and next work

Acceptance requires host source/origin/plan/copy/oracle mutation tests,
freestanding lint and feature precedence, emitted entry/return and complete
stack-path inspection before VMs, and strict bounded serial transcripts under
QEMU TCG/KVM at 96/320 MiB where available. The host must corroborate the reported
SYSCALL bytes against the exact retained interpreter source, not merely trust a
guest success flag. Ordered records must distinguish initial facts, actual raw
boundary and complete retirement, including no caller return and no Linux exit.
Keep attempted output and capability refusals separate from passing evidence.

The provisioning path is QEMU-specific. A VirtualBox boot lacking this exact
input interface cannot qualify this execution boundary; report that limitation
explicitly instead of substituting different bytes or claiming a pass. Run all
repository-required and affected regressions, preserve default-image exclusion,
review changed baseline bytes, and serialize builds/VMs under the memory guard.
The qualification record binds source, inputs, artifacts, failed attempts,
stack evidence and resource restoration to the measured results.

Qualification passed under TCG/Nehalem and KVM/host at both memory sizes. The
unchanged interpreter reached actual syscall 12, `brk(0)`, at `0x600284c9`;
the veil selected unsupported retirement, with no caller return or Linux exit.
All sixteen required targets, thirteen affected regressions and an additional
fresh CPU-workspace regression passed. Three provisioning-negative boots
rejected missing, shortened and corrupted source inputs. VirtualBox booted but
refused `application_cpu_amd_error_pointers` before userspace and was powered
off. All five baseline images were compared and changed bytes independently
reviewed. The final evidence audit passed with no observed OOM. The
[Linux Loader Entry Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Entry%20Log.md)
records exact hashes, bounds, failures, memory observations and their limits.

Next, answer the measured `brk(0)` query in the userspace veil through the
existing authenticated reply path and resume the unchanged interpreter to its
next actual service boundary. Then implement only the services required by that
path: Linux descriptor/file lookup and reads, address-space growth/mapping and
protection, then dependency/TLS startup and continued execution through libc to
an actual `true` exit. These policies remain in the userspace veil over separately
qualified native mechanisms. This increment supplies neither a general Linux
runtime nor Electron/Antimatter launch, rendering or input.

## Reference

- [QEMU fw_cfg specification](https://www.qemu.org/docs/master/specs/fw_cfg.html):
  selector/data I/O, directory layout, sequential reading and external read-only
  file provisioning.
