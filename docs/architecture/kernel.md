# Kernel foundation architecture

## Retained workspace admission

Proposed [ADR 0067](../adr/0067-qualify-retained-native-workspace-admission.md)
adds a separate `owned_workspace_test` profile over one retained prepared
launch. Trusted selection admits exactly 32 RX pages, eight RW/NX data pages
and a 64 KiB user stack, with surrounding guards and an exclusive 16 KiB
transition stack. Earlier application and workspace limits are unchanged.
The complete workspace/compositor keeps its GPR/CR0.TS contract and fixed
`INT 0xf3` mailbox; this does not select the eager application CPU profile.

The owner retains 69 frames in five leases, plus the 576-frame private display
stage. Every gate binds the exact frame, root, transition stack, running task
and first-page mailbox alias. A fixed copied header and the existing pure
decoder support that supervisor high alias without widening the legacy
identity-mapped transport. Presentation temporarily selects bootstrap CR3 in
an IF-clear scope, restoring the exact retained root before returning to user
code. Terminal withdrawal restores the bootstrap continuation and CPU/stack
state before clearing storage and reclaiming mappings.

Separate guard-write and NX-execution admissions retire before the interactive
desktop starts. Ordered lifecycle records check zeroing, wide data/stack probes
and exact resource restoration. The
[qualification log](../../notebook/Development/2026-10-05%20Owned%20Workspace%20Admission%20Log.md)
records static review and guest evidence. The native editor bridge, three-owner
scheduling, storage service routes and cross-task focus policy remain separate
integration work.

## Eager workspace CPU ownership

[ADR 0068](../adr/0068-qualify-eager-workspace-cpu-ownership.md) adds the separate
`owned_workspace_cpu_test` profile. It uses the retained workspace layout with
one resident x87/MMX/SSE2 image, the original bootstrap CPU image and a clean
handler image. A private observation image checks that capture actually leaves
the handler in its clean state. The legacy GPR-only workspace remains available.

Every user gate, IRQ and supported exception captures the current image before
policy processing. Returning to the user restores that image; a kernel-origin
IRQ during the integer-only idle continuation leaves the user owner alone.
Terminal withdrawal restores the bootstrap root and TSS stack before restoring
and verifying the original CPU state and controls, wiping image storage, and
releasing the retained execution resources. An uncertain invariant fails closed.

Four sealed startup transitions check distinct x87 and MMX patterns, all sixteen
XMM registers, MXCSR and clean initial state. Both stable patterns must survive
at least two real user timer captures. Only exact inspected assembly in the
probe may use these instructions; the Rust workspace and kernel stay soft-float.
Setup intervals preserve intermediate images without claiming the final pattern
is already installed. The [qualification log](../../notebook/Development/2026-10-05%20Eager%20Workspace%20CPU%20Log.md)
records exact artifacts, observed paths and limits. This prepares CPU composition
for the later workspace/editor/service coordinator; it does not implement it.

## Status

This document separates mechanisms observed in the current x86-64 kernel from
the intended circle and vessel architecture. The diagnostic fault spine and
the bounded feature experiments accepted by ADRs 0011 through 0015 are
implemented, as are ADR 0018's fixed supervision/restart proof, ADR 0019's
fixed native rite-gate proof, ADR 0020's fixed composition of rites,
scheduling, failure containment, and one-restart supervision, and ADR 0022's
fixed native session-input and exact-wake bridge. ADR 0024's bounded ring-3
display-presentation proof and ADR 0028's one-slot native semantic-surface
transport proof are also implemented, as is ADR 0029's bounded composition of
that transport with one isolated renderer, private full-frame staging,
framebuffer presentation, and host-gated semantic publication. Proposed ADRs
0041–0044 add inert memory planning, retained owned construction, bounded
resident-root activation, and actual GPR-only user entry over low-address roots.
All 24 validation targets passed for the ADR 0044 increment, including two-size
TCG/KVM memory proofs. The [Owned User Execution Log](../../notebook/Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records the final evidence and limitations.
Proposed [ADR 0046](../adr/0046-prove-retained-prepared-launch-ownership.md)
adds retained preparation without a task, explicit ready-task publication at
commit, and retryable cancellation/terminal cleanup for that same owned profile.
The [Prepared Launch Ownership Log](../../notebook/Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records its completed validation. `cancel_unstarted` permits only a ready task
without an attached context to become terminal before resource reclamation;
running/scheduled tasks retain their existing lifecycle rules.

The preceding proposed
[ADR 0047](../adr/0047-prove-sustained-dispatch-over-retained-launches.md)
composes two retained launches with sustained scheduling over those roots.
The aggregate retains 36 frames (18 data, 18 tables), ten bound leases, two
16 KiB supervisor high-alias privilege stacks, and activation/cleanup progress.
A narrow unstarted admission rollback rejects any global current task or
reservation, preserves survivor queue order and sequence cursors, then leaves
task cancellation to the launch owner. Native evidence covers partial pair
admission, two failed stack clears with retained retries, 64 real preemptions,
one exact UD2 fault, and 16 survivor ticks. Nested user RSP is stack top minus
24 bytes. The faulted owner remains retained until both tasks are terminal.
Clear/release cleanup is retryable; failed mapping revalidation retains the
owner fail-closed and does not promise recovery on another `finish` call.
Focused tests, feature Clippy, and 96-MiB/320-MiB TCG/KVM proofs passed.
All 26 targets qualified across documented phases, with 258 final frozen files
unchanged after a narrow display-capture harness correction. The
[Owned Sustained Dispatch Log](../../notebook/Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
records the final evidence and interrupted-run diagnostic limit.
That GPR-only fixture does not define a stable application ABI or general loader.

The preceding proposed
[ADR 0048](../adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
adds a separate internal application profile over those retained owners.
Supervisor-only aligned images eagerly own x87/SSE2 state at every user
transition, preserve the original kernel state and controls, and are cleared
before their owner is released. CPU admission excludes unsupported AMD error
pointer behavior, enabled XSAVE/TLS controls and unowned fast syscall entries.
Copies bind the exact current task/circle generations, root, stack, permissions
and retained page-table storage; each operation is limited to 4096 bytes and
preflight completes before any destination byte changes. Selected CPL3
#DE/#UD/#GP/#PF classification is separate from survivor return admission.
The last current task may fault into the retained bootstrap continuation before
its abandoned exception stack and mappings are reclaimed.

All 28 validation targets passed on 265 unchanged frozen source files, including
96-MiB/320-MiB TCG Intel-baseline and actual AMD-host KVM proofs. Each profile
run accepted 16 records and restored 24,146/81,490 free frames; normal and
workspace images match the preceding checkpoint. The
[Application Profile Log](../../notebook/Development/2026-10-03%20Application%20Profile%20Log.md)
records final evidence and limits.
This does not qualify arbitrary untrusted applications, #MF/#XM recovery,
TLS, AVX/XSAVE, permanent kernel placement, or global W^X.

The preceding proposed
[ADR 0049](../adr/0049-qualify-a-bounded-freestanding-ny-object.md)
qualifies one pinned Ny scalar object under that profile. Its inspected startup
shim checks initialized data, zero-filled globals and stack alignment; the kernel
requires timer observations inside generated code, checked result/message copies,
and failure-to-peer continuity before complete owner and CPU-state cleanup.
All 30 targets passed on 272 unchanged frozen files, including 96-MiB/320-MiB
TCG and actual AMD-host KVM proofs. Each run accepted 13 records and restored
24,149/81,493 free frames. The
[Ny Object Log](../../notebook/Development/2026-10-03%20Ny%20Object%20Log.md)
records final evidence and limits.
This fixed composition supplies no general compiler target, hosted runtime or loader.

Proposed [ADR 0050](../adr/0050-prove-a-bounded-native-application-contract.md)
implements the preceding experimental application channel composition. Exact
producer/service task generations bind two opposite copied channels; editor
state and pixels remain opaque to the kernel. Copies preflight before channel
commit, and close revokes delivery before terminal task transitions. A bounded
Rust client and service retain separate roots, x87/SSE2 state and stacks. The
service stages copied rows after the producer exits; framebuffer access occurs
only after bootstrap restoration and both tasks retire. All 33 matrix jobs
passed on 291 unchanged frozen files, including two-size TCG/KVM and capture
proofs; see the [Native Application Log](../../notebook/Development/2026-10-03%20Native%20Application%20Log.md).
That checkpoint supplies scripted input and deferred presentation, not a full
editor, physical input routing or stable syscall contract.

The completed checkpoint under proposed
[ADR 0051](../adr/0051-prove-userspace-elf-loading-against-native.md) adds a
feature-private userspace ELF loader. The ring-3 vessel parses an immutable
sectionless two-segment ELF64 image and offers a copied 96-byte plan. The kernel
decodes only that plan, requires exact agreement with the trusted host manifest,
and checks the current loader role; ELF headers and format policy stay outside
the privileged path. Loader teardown precedes application allocation. Retained
owners then populate and read back RX/initialized RW bytes, verify zero-fill,
and run the same native editor/service scenarios through flat and ELF-derived
spans. The routes must agree on service counters, surface hashes and final
pixels while restoring their ownership. All 35 matrix jobs passed on
2026-10-04 with 302 frozen source files unchanged, including two-size TCG/KVM
ELF and native proofs plus captures. The
[ELF Application Log](../../notebook/Development/2026-10-03%20ELF%20Application%20Log.md)
records this bounded qualification; ADR 0051 remains Proposed.
That checkpoint retains the native `int 0xf4` personality and does not admit
arbitrary ELF files.

The preceding proposed [ADR 0052](../adr/0052-prove-linux-startup-and-record-device-syscalls.md)
qualifies owned SYSCALL entry/MSR state for one inspected Linux producer while the peer service keeps native INT f4. A trusted supervisor
stack is selected before any push; checked IRETQ and eager CPU ownership remain
shared. All 37 matrix jobs and two restoration jobs passed on 313 unchanged
frozen source files, including two-size TCG/KVM boots and matching captures.
Generic channel consumption copies and releases a record atomically;
the fixed read/write/poll adapter adds no editor or pixel policy. This narrowly
scoped personality lives only in that feature-private proof controller; its
qualified evidence remains independently reproducible.

The preceding proposed [ADR 0053](../adr/0053-prove-a-supervised-userspace-syscall-veil.md)
implements raw-call forwarding to a third retained userspace task. A move-only
scheduler authority binds owner, task generations, blocked caller, current veil
and reserved reply dispatch while preserving the native service queue. Checked
caller copies retain both roots and validate every span through supervisor
aliases. The veil owns syscall/FD/errno/poll policy; the kernel exposes only
native route snapshots, copied reads, staged submit/consume/copyout and exact
completion. A rejected completion preserves its pending stage; a veil fault
revokes authority, contains the dependent caller and dispatches the service.
The old kernel policy adapter is excluded from this profile. All 39 matrix jobs and two restorations passed; the [Userspace Veil Log](../../notebook/Development/2026-10-04%20Userspace%20Veil%20Log.md)
tracks the three scenarios. Fixed startup remains proof-controller policy,
and one caller/veil relation does not establish a general Linux process ABI.
ADR 0054 below extends this fixed caller relationship.

The current proposed [ADR 0054](../adr/0054-prove-a-shared-multi-caller-userspace-veil.md) qualifies two independent Linux editor
producers and two native services sharing one supervised veil. Exact sealed
membership, per-caller routes and copied-memory isolation preserve the native
baseline. A caller fault leaves its peer progressing; a veil fault revokes both
dependents and discards its staged effect while both native services survive.
All 41 qualification checks and two image restorations passed, including TCG/Intel and
KVM/AMD at both guest sizes with capture comparison. All twelve preceding
embedded artifact blobs remain unchanged and no OOM occurred. The
[Shared Veil Log](../../notebook/Development/2026-10-04%20Shared%20Veil%20Log.md) records the frozen-source and compiled-code evidence.
This is a fixed two-member serial service, not full Antimatter/Electron or a
general process/runtime service. Native runtime facilities remain the next gate.

General address spaces, production scheduling, multitask circles, a stable external control
ABI, authenticated principals, production vessel supervision, and a semantic
desktop remain designs.

## Diagnostic fault spine

The kernel takes ownership of descriptor state immediately after COM1 is
initialized and before it follows addresses carried by `boot_omen`:

1. construct and load a kernel GDT with 64-bit code, data, and TSS descriptors;
2. load the task register from the kernel-owned TSS;
3. populate interrupt-stack-table entries backed by fixed 16 KiB stacks for
   double faults and non-maskable interrupts;
4. install interrupt gates for all 32 architectural CPU exception vectors; and
5. emit `kernel.fault_path.ready` with the installed-vector and stack state.

The TSS also carries a separate fixed 16 KiB ring-transition stack. Normal
images do not enter ring 3. The `circle_test`, `control_test`, and owned-user
`memory_test` phases use and clear this stack for bounded transition proofs; its presence is not evidence of
a general scheduler, circle, or vessel.

NMI remains masked after the IDT is installed. Its gate and dedicated stack are
prepared, but unmasking waits for deterministic NMI injection, nested-fault
behavior, and VirtualBox evidence. Maskable external interrupts remain disabled
until the later `legacy_pc` initialization has installed returning gates,
established controller ownership and acknowledgement, and initialized every
enabled source.

## Exception-entry contract

Narrow assembly stubs normalize CPU frames by placing a vector and error code
at fixed offsets. Stubs for exceptions that already receive a processor error
code do not synthesize a second one. The common entry clears the direction flag,
saves every general-purpose register, aligns the stack for the System V ABI,
and calls the Rust dispatcher. The complete long-mode frame is 22 words
(176 bytes): fifteen GPRs, vector/error, and hardware RIP/CS/RFLAGS/RSP/SS.
The snapshot reads actual hardware RSP and SS for CPL0, CPL3, and IST entries;
it never reconstructs interrupted RSP from the exception-frame address. Its
legacy optional SS representation receives `Some(saved_ss)` for every valid
long-mode hardware snapshot. Reported `stack_switched` still describes a
privilege or IST switch, not the presence of saved SS/RSP.

Double fault uses IST 1 and NMI uses IST 2. Other kernel-origin exceptions use
the current kernel stack; user-origin exceptions select TSS.RSP0. Unexpected
exceptions in the normal image are fatal. Feature-local handlers may return an
exactly validated frame to the common assembly return path or consume a known
terminal continuation; those handlers clear the active-exception latch first.
The resident proof resumes only its armed CPL0 protection probes, and the
owned-user proof resumes only its exact known reports or terminates the selected
task. Existing circle and scheduler containment retains its separate profile.
A nested entry emits a minimal fatal event and halts. None of these exact
exceptions supplies implicit production recovery or general user-copy policy.

The structured `kernel.exception` record contains:

- vector and a static exception name;
- processor error code;
- instruction, interrupted stack, code-segment, and flags state;
- kernel or user origin derived from the saved selector;
- disposition; and
- for page faults, CR2 plus decoded access-cause bits.

Addresses make early bring-up failures attributable with `build/kernel.debug`.
They are security-sensitive diagnostic data once address randomization, secret
workloads, persistence, or remote telemetry exist; later oracle policy must
govern their disclosure and retention.

## Deterministic evidence

The `fault_test` feature implies the existing QEMU smoke-test transport and is
absent from normal images. It arms one exact tuple—invalid-opcode vector,
instruction address, and kernel code selector—then executes a dedicated `ud2`.
Only a frame matching that tuple can emit `kernel.fault_test.passed` and request
the test-only QEMU success exit. Any other exception takes the fatal path.

`make fault-smoke` requires all of these observations within the bounded QEMU
deadline:

- the ordinary `boot.ready` event;
- `kernel.fault_path.ready`;
- an attributed `kernel.exception` for vector 6; and
- the exact `kernel.fault_test.passed` event.

The regular boot and shell smoke tests run separately, proving that test-only
fault injection did not enter the normal image.

## Fixed circle experiment

[ADR 0011](../adr/0011-prove-the-first-circle-with-a-fixed-x86-64-prototype.md)
specifies the implemented bounded x86-64 proof. A `circle_test` image builds a
separate page-table root, retains supervisor-only kernel identity mappings, and
maps one fixed ring-3 window with RX code, RW+NX scratch and stack pages, and
absent guards. It enables and reads back NX and supervisor write protection
before user entry.

The proof uses three clean generations:

1. enter ring 3, report through one numeric test-only gate, and prove that a
   present supervisor canary read fails with the exact user-read protection
   fault;
2. rebuild from cleared storage, enter ring 3 again, and prove that instruction
   fetch from the present NX stack fails with the exact fetch fault; and
3. rebuild once more, observe clean scratch, make a terminal report, and tear
   the generation down.

Containment is valid only when the lifecycle phase, faulting instruction,
address, selectors, stack bounds, processor error code, and scratch marker all
match. After each generation, the original CR3 must be restored before its
private page tables and code, data, stack, and privilege-stack residue are
cleared with volatile writes and verified as zero.

Host checks, strict `make circle-smoke` validation, normal-image exclusion,
regression boots, and a complete VirtualBox trace passed on 2026-09-09.
VirtualBox saves the page-fault resume flag while QEMU does not, so the exact
matcher permits only that single architectural difference. Its fixed
addresses, vector, reports, mappings, and recovery trampoline remain removable
test machinery, not ABI.

## Bounded bootstrap memory ownership

The normal bootstrap processor copies Threshold's complete version-2 memory
map and normalizes it into a fixed two-bit ledger covering 4 KiB frames below
four GiB. Usable records align inward; blockers and reservations align outward;
enabled non-usable overlap always wins. The manager reserves everything below
one MiB, the linker-defined kernel memory extent, and the validated framebuffer
aperture. It refuses publication unless every linked kernel frame is covered by
exact enabled volatile type-1 memory and the framebuffer is disjoint from low
and kernel memory.

An opaque `frame_lease` carries a nonwrapping allocator-instance identity,
fixed record slot, nonwrapping generation, extent, and data-or-page-table
purpose. Allocation is deterministic lowest-fit with power-of-two frame
alignment. Foreign, bound, stale, altered, duplicate, and inactive leases
cannot release frames. Allocation and release both volatile-clear and verify
their complete extent; a failed release remains owned rather than exposing
residue as free memory. Those operations and the raw clearing primitive require
an explicit unsafe authority contract over the complete physical extent.

Shared x86-64 paging code enables and reads back `EFER.NXE` and `CR0.WP` during
normal boot. The feature-only address-space owner borrows the existing
supervisor low branch and owns a four-frame high mapping branch. It rejects
writable-plus-executable leaves, requires exact table walks before activation
and teardown, tolerates only processor-maintained accessed/dirty changes, and
will not withdraw or destroy the active root. Mapping consumes the noncopyable
data lease; only successful unmapping returns it, preventing a caller from
releasing a frame while the page tables still name it.

The original `make memory-smoke` phase boots one feature image at 96 MiB and
320 MiB. At each size it allocates distinct data and table leases, maps one fixed virtual address to
different supervisor RW+NX frames in two roots, switches A-to-B-to-A, restores
the original CR3, destroys both roots, and reacquires the same data frames as
verified zero. The trace also explicitly records rejection of virtual overlap,
duplicate physical binding, and active-root destruction. A strict event parser
required complete ordered boot prerequisites, the exact eight-record ownership
evidence set (`kernel.memory.ready` plus seven feature-test records), no error
or fatal record, and full free-count restoration at the ADR 0012 checkpoint.
Subsequent phases below extend that exact event sequence; normal-image
inspection continues to reject the test namespace, symbols, and markers.

QEMU reported 24,162 free frames at 96 MiB and 81,506 at 320 MiB, an exact
57,344-frame increase. The dedicated 512 MiB VirtualBox VM completed the same
lifecycle with 130,675 free frames and no error or fatal record. At that
checkpoint, the normal image was restored with 130,686 free frames and observed
running after `boot.ready` and `bootstrap_shell.ready`. ADR 0012 is accepted on
this evidence.

This manager is BSP-only, fixed-capacity, single-publication, and kernel-private.
It is not a heap, stable allocator ABI, general mapper, user address-space
service, or circle. Threshold's original supervisor identity map still aliases
all physical memory below four GiB writable and executable, so the proof
establishes bookkeeping and lifecycle correctness rather than isolation from
compromised ring 0. Firmware remains trusted to describe physical memory
truthfully after structural validation.

## Retained mappings and owned low-address user execution

Proposed [ADR 0041](../adr/0041-plan-bounded-user-mappings-and-physical-access.md)
separates checked multi-region user geometry and privileged physical-window
arithmetic from allocation. [ADR 0042](../adr/0042-prove-inactive-owned-user-mapping-construction.md)
constructs exact inactive trees with retained table/data leases, quarantine on
failure, and retryable cleanup. [ADR 0043](../adr/0043-prove-resident-mapping-activation.md)
adds explicit supervisor section/stack residency below 2 MiB and a retained
session that borrows two mixed owners plus their high physical alias through
root selection. Its six exact CPL0 faults test read-only, guard, and NX
permissions while a colliding low virtual address leaves privileged physical
access intact. That phase remains distinct from user execution.

Proposed [ADR 0044](../adr/0044-prove-owned-low-address-user-execution.md) adds the
[`user_entry_plan`](../../kernel/src/arch/x86_64/user_entry_plan.rs) and
[`user_mapping_test`](../../kernel/src/arch/x86_64/user_mapping_test.rs) proof.
Two real task/circle identities each retain three mappings: one RX code page
at 4 MiB, two RW+NX private data pages at 8 MiB, and two guarded RW+NX stack
pages. Each root owns nine table frames and five data frames. Eight leases retain
the total 28 frames through bootstrap → B → A → bootstrap.

Entry admission uses one declared initialized executable-byte interval and a
16-byte-aligned initial stack with bounded headroom; resumed RSP must be
8-byte aligned and stay inside the admitted stack. B resumes two exact private
UD2 reports at different noninitial stack pointers, including a nested call,
then terminates on a precisely matched user read of supervisor kernel text
(error 5). A subsequently completes its two reports and exact exit. Matching
binds live task/circle/root, phase, initialized instruction membership,
selectors/flags, the complete resident frame, and actual saved user stack.
The fixture preserves distinct private data at identical virtual addresses.

This GPR-only profile admits zero TLS bases, brackets entry with CR0.TS,
preserves TSS.RSP0, and wipes the dedicated privilege stack after each terminal
return. No timer scheduling occurs. The root session prevents owner cleanup
until bootstrap restoration and borrower withdrawal; mapping reclamation and
child detachment precede task/circle destruction. Final checks require retired
generations, zero attached contexts, cleared continuations, removed physical
window, and boot-equal frame accounting. Unexpected failures halt with retained
owners rather than silently dropping cleanup responsibility.

The extended memory transcript has 23 structured records and 20 selected
lifecycle records. Exact keys/types/values and mutation tests cover the three
new `user.ready`, `user.verified`, and `user.released` records; normal-image
inspection rejects the Rust proof and assembly/payload symbols. TCG and KVM
proofs passed at 96 MiB and 320 MiB, restoring 24,150/81,494 free frames.
All 24 regression targets passed on 240 unchanged frozen source/configuration
files. Focused checks passed 107 x86 tests, eight memory-transcript tests, and
freestanding Clippy; full suites passed 398 default kernel, 441 all-feature
kernel, and 92 xtask tests. The
[Owned User Execution Log](../../notebook/Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records final hashes, including changed normal/dispatcher/workspace images from
the shared exception-snapshot correction.

The handshake is private proof machinery, not a syscall ABI. This does not
migrate the scheduler/desktop or the legacy mapper's failure-retention gap,
provide general launch/user-copy/fault policy, own FPU/TLS state, or select a
permanent kernel layout. The high supervisor alias can still write executable
physical storage, so no global W^X claim follows.

## Bounded interrupt, time, and input delivery

[ADR 0013](../adr/0013-prove-bounded-interrupt-time-and-input-delivery.md)
specifies the accepted BSP-only proof. One x86-64 IDT owner installs returning
assembly stubs for vectors `0x20` through `0x2f` without weakening the separate
exception admission and fatal fallback. The stubs preserve every general-purpose
register, clear the direction flag for Rust, normalize and align the call frame,
and return with `iretq`. Interrupt gates keep nesting disabled during the first
bounded handlers.

A replaceable `legacy_pc` backend begins with both 8259 PICs masked, remaps them,
programs PIT channel 0 with command `0x34` and divisor bytes `0x9c`, `0x2e`
(11,932), initializes the 8042 under bounded poll limits, and then unmasks only
IRQ0 and a healthy IRQ1. NMI remains masked.
PIC handling distinguishes spurious IRQ7/IRQ15 and acknowledges real cascaded
interrupts in slave-then-master order. Repeated empty keyboard IRQs degrade the
input source and mask IRQ1 rather than spinning indefinitely. A successful
initial setup or rearm resets that streak before IRQ1 is enabled again.

PIT IRQs advance immutable boot timebase generation 1. Its ticks are monotonic
and comparable within that identity; different timebase generations are not
ordered by their numeric ticks. They do not broadcast periodic events or imply
wall-clock time, and this slice does not reprogram or rotate the timebase.
Keyboard IRQs publish into a fixed 64-entry, single-producer/single-consumer
channel as typed physical observations carrying source generation, nonwrapping
source-local sequence, and monotonic observation time. A full channel drops the
newest attempted record but retains an exact loss latch; the consumer receives
that loss before any later transition and must reset derived key state before
explicitly rearming. Source generation, sequence, tick, and loss-count
exhaustion are terminal rather than wrapping identities.

The scan-set-1 decoder owns prefix, make/break, malformed-stream, and physical-
key framing state. The surrounding input transport owns explicit loss and its
controlled reset boundary; after loss, the architecture completes a bounded
8042/device reinitialization before installing the next source generation at
sequence zero. Steady-state bytes `00`, `ee`, `fa`, `fc`, `fd`, `fe`, and `ff`
are transport/response failures that clear partial framing and force that loss
and rearm path rather than becoming key transitions. Ambiguous `aa` retains its
ordinary left-Shift-release meaning. A different adapter outside IRQ context
maps physical transitions to the temporary shell's US ASCII editing input. IRQ context never
allocates, formats, dispatches commands, follows user pointers, or publishes
scan bytes, physical keys, or derived text. The shell uses an interrupt-safe
readiness check plus `hlt`, so it neither polls the controller nor misses the
check-to-sleep race. On typed loss it clears its unfinished line and layout
state, records only aggregate identity/count metadata, and requests that
transport reinitialization and rearm.

The normal readiness record exposes controller configuration, masks, timebase
and source generations, queue capacity, NMI state, and IF state without input
content. A fatal IRQ record forces a fresh serial record boundary before its
static JSON so an interrupt cannot splice it into foreground output. COM1
remains output-only. Physical input confers no principal or authority, and AI
clients have no synthetic-key entry. ADR 0014 now proves one fixed
kernel-authorized IPC path for coequal human and machine fixtures, but
production machine control still requires authenticated identity, policy, and
versioned invocations above that mechanism.

`make interrupt-smoke` injected 12 press/release pairs through QEMU and observed
readiness with initial `0xff`/`0xff` and active `0xfc`/`0xff` masks, eight real
PIT IRQs advancing tick 0 to 8, exact generation-1 sequences 0 through 23,
return from both handlers with preserved register state, deterministic
one-record loss at generation-1 sequence 88, IRQ1 containment at master mask
`0xfe`, bounded 8042/device reinitialization, generation 1-to-2 rearm at master
mask `0xfc`, and generation-2 sequences 0 through 1 before final `0xff`/`0xff`
masks. The 512 MiB VirtualBox run reproduced that lifecycle with ticks 1 through
9 and 130,674 free feature-image frames. Neither strict transcript contained
private input, an error, or a fatal record. The verifier accepts only the exact
ordered record schemas and rejects nonempty unstructured lines, malformed
records, unexpected events, and unknown fields. Normal-image inspection rejected
all named feature-only helpers, the event namespace, and the privacy probe before
the ordinary image was restored with 130,683 free frames. That VM corrected
`versiom` with Backspace plus `n`, dispatched `version` through IRQ1, emitted the
next prompt, and remains running.

This is not a scheduler, deadline service, stable time/input ABI, user-space
driver model, or production platform abstraction. It trusts the emulated legacy
devices, runs on one bootstrap processor, cannot prevent interrupt denial of
service, has no APIC/ACPI topology or SMP routing, and provides neither secure
attention nor user authentication.

## Fixed attributable task, sigil, and IPC proof

[ADR 0014](../adr/0014-prove-fixed-attributable-sigil-gated-ipc.md) specifies
the accepted `control_test` proof. Its architecture-neutral core owns fixed
tables for 16 circles, 16 tasks, four endpoints, 16 sigil records, four local
bindings per task, and four revocation lineages. The control endpoint has two
fixed message slots. Live circle, task, endpoint, sigil, binding, and lineage
identities carry nonzero generations that never wrap or silently become valid
again. Fresh table validation, child accounting, dependency-ordered withdrawal,
and complete volatile record clearing constrain stale identities and reuse.
These capacities and representations are pre-alpha proof parameters, not ABI.

Each proof circle owns at most one task, one allocator-backed address-space
root, and exactly three user mappings: RX code, an RW+NX fixed mailbox, and an
RW+NX stack. Runtime table walks verify their effective permissions and verify
the surrounding fixed guard addresses are absent. The kernel sequencer permits
only one globally running task and drives it to completion with maskable
interrupts disabled and NMI masked. This is serialized one-shot BSP execution,
not scheduling, preemption, blocking, or general process support.

A user-visible sigil reference contains only the `SIG1` format tag, a local
slot, and a binding generation. Resolution begins in the current task's binding
row and then checks the fresh kernel task identity, holder circle and fixture
principal, kernel-only sigil identity, exact endpoint, required known rights,
and live lineage. Copying reference bits between tasks therefore cannot copy
authority. Root issuance remains kernel-sequencer-only; explicit transfer is a
failure-atomic move, attenuation cannot broaden rights, and serialized lineage
revocation denies every later lookup. The IPC table exposes composite
authorized send and receive operations, so crate users cannot bypass sigil
validation through an unguarded transport call.

An accepted send copies at most 32 payload bytes into a kernel-owned FIFO slot,
assigns a nonwrapping sequence only after acceptance, and stamps task, circle,
fixture principal, and origin from a fresh kernel task-table lookup. It retains
no user pointer or shared sender alias. A full queue and an empty queue return
typed `would_block` results without partial mutation. Receive revalidates the
receiver and exact receive authority, preflights the fixed mailbox, commits the
copy through its allocator-authorized supervisor physical alias, and only then
clears and consumes the queue head. A failed copy leaves the message available;
successful delivery preserves FIFO, and sender teardown cannot alter an already
accepted message.

The x86-64 harness binds those mechanisms to real CPL-3 execution through one
feature-only DPL-3 interrupt gate. Every call must match the current task and
lifecycle phase, exact post-`int` instruction address, selectors, sanitized
flags, stack bounds, operation, and fixed mailbox. User state never chooses its
task, circle, principal, origin, endpoint ownership, or physical address. User
entry clears unspecified registers and FS/GS, sets `CR0.TS` to deny inherited
x87/MMX/SIMD state, and uses `IF=0` and `IOPL=0`. Nonterminal calls return with
`iretq`; the exact completion call abandons the user frame through a single-use
kernel continuation.

Exact completion restores the pre-entry `CR0.TS` state and saved Rust stack
while abandoning the user frame. The kernel then selects its original CR3,
clears the active task context, and wipes the TSS transition stack. Lifecycle
teardown invalidates the task's sigil authority, withdraws any owned endpoint,
removes and releases the three mappings and address-space root, and only then
destroys the task and circle records. Code, mailbox, stack, queue slots, table
records, and saved continuation cells are volatile-cleared and verified before
reuse. The human and machine producer fixtures execute byte-identical ring-3
code through the same gate, authorization, queue, return, and teardown path.
Their kernel-selected synthetic principals and origins remain distinct in the
two FIFO messages. Those fixture labels demonstrate equivalent mechanism and
attribution; they do not authenticate a person, model, agent, or device.

Acceptance on 2026-09-09 included `make check` with 193 kernel tests and every
QEMU regression. The strict QEMU control run accepted exactly 28 records and
restored 65,114 free frames. The exact VirtualBox VM
`Anomalous Dev [Threshold]` accepted the same 28-record lifecycle and restored
130,667 free feature-image frames. `make normal-exclusion` rejected the gate,
payload, fixture, private marker, feature event namespace, and test-only control
paths in the ordinary image. That normal image was then restored and observed
running with 130,683 free frames.

This proves a narrow composite authority and copying boundary, not production
isolation. The sequencer, synthetic roles, firmware, emulator, and all kernel
code remain trusted. Threshold's writable and executable supervisor identity
alias remains present; NMI remains masked; only one BSP task can run; and there
is no scheduler in that `control_test` image, general syscall or IPC ABI,
vessel, authentication service, DMA isolation, IOMMU policy, denial-of-service
containment, or microarchitectural isolation.

## Bounded preemption and user-fault containment

[ADR 0015](../adr/0015-prove-bounded-preemption-and-user-fault-containment.md)
accepts a separate `scheduler_test` proof built on the earlier task, circle,
memory, interrupt, and fault mechanisms. Its architecture-neutral fixed
round-robin table admits exactly two generational task identities with attached
contexts. Checked composite transitions move tasks among ready, running,
suspended, faulted, and exited states without exposing a half-applied switch.
New sigil destinations may be ready or running; suspended tasks retain state
but cannot exercise authority until they run again.

Each one-task circle owns a private root, RX code, RW+NX mailbox and user stack,
four page-table frames, and a distinct allocator-backed 16 KiB supervisor
privilege stack. Initial, interrupt, and exception contexts share one exact
22-qword x86-64 layout. Every dispatch revalidates the selected task identity,
CR3, frame extent, selectors, RFLAGS, user RIP/RSP bounds, and TSS RSP0
ownership while interrupts are masked. `CR0.TS` remains set throughout user
execution so the proof cannot inherit x87/MMX/SIMD state.

Exactly two real PIT IRQ0 deliveries preempt the faulting task and then its peer.
The first task resumes, verifies private state, and executes one fixed `ud2`.
Only the exact vector, error, active task/root/frame, selectors, instruction,
stack, and task-local state may fault that task and resume the peer; every
unexpected scheduler-active exception takes a static terminal path without
emitting attacker-selected addresses or registers. The peer then verifies its
own continuity and completes through a single-use saved kernel continuation.

Successful teardown restores the kernel CR3, bootstrap TSS RSP0, original TS
state, and saved kernel stack, and places both PICs in the declared all-masked
terminal state before withdrawing contexts, mappings, roots, allocator leases,
tasks, and circles. User, page-table, and privilege-stack storage is cleared and
verified; inactive address-bearing metadata is scrubbed; and all accounting
returns to its initial value. Setup or teardown inconsistency instead halts and
quarantines any ownership not conclusively withdrawn. The proof therefore
establishes complete successful cleanup, not rollback or retry after partial
failure.

Acceptance on 2026-09-09 included `make check` with 3 ABI tests, 214 base kernel
tests, 220 scheduler-feature kernel tests, and 26 verifier tests, plus every
existing QEMU regression. QEMU accepted exactly 25 records at PIT ticks 1 and 2
and restored 65,112 free frames. The exact 512 MiB VirtualBox VM accepted the
same lifecycle at ticks 2 and 3 and restored 130,665 free frames; its sole
additional exception-frame variance was the architectural Resume Flag (`RF`) in
RFLAGS.
Normal-image exclusion passed before the ordinary image was restored and left
running at `bootstrap_shell.ready` with 130,683 free frames.

This is not a production scheduler, fairness or deadline policy, arbitrary task
or vessel runtime, stable syscall/context ABI, general fault recovery, SMP
implementation, FPU/SIMD switch, TLS/per-CPU-base policy, authentication
service, or denial-of-service boundary. The completion continuation abandons
Rust gate frames and is valid only while they contain no destructors, RAII
guards, or lasting borrows. NMI remains masked, only the BSP executes, APs are
not started, privilege stacks have no guards, and the writable/executable
supervisor identity alias remains a ring-0 bypass.

## Bounded vessel supervision and restart

[ADR 0018](../adr/0018-prove-bounded-vessel-supervision-and-restart.md)
accepts a separate feature-only x86-64 experiment. One persistent supervisor
fixture blocks, observes the exact contained failure of vessel generation 1,
authorizes one destructive restart from a safe kernel-stack checkpoint, then
observes generation 2 complete. The proof withdraws the failed generation
before constructing the replacement from cleared storage and restores all
scheduler, supervision, task, circle, mapping, and allocator accounting.

That fixed relationship proves bounded blocked-task, observation, and restart
mechanics. It is not a general vessel manager, service loader, package lifecycle,
health policy, arbitrary exception recovery path, or production supervisor.

## Accepted fixed x86-64 rite-gate proof

[ADR 0019](../adr/0019-define-first-supervised-vessel-rite-abi.md) accepts a
removable `rite_test` image for the native x86-64 kernel. It is not an IA-32
kernel target, stable syscall ABI, or foreign application personality. A later
user-space compatibility veil for IA-32 machine code and the 32-bit Win32 ABI
must contain foreign pointer widths, handles, PE/NT conventions, and calling
conventions in user space, then translate them into the same fixed-width native
rites used by every other client.

The proof admits one fixed ring-3 shell-task fixture, modeling a future
supervised vessel, through DPL-3 interrupt vector `0xf3`. It does not create a
supervision relationship. The user mapping contains only RX code, one fixed
RW+NX mailbox, and an RW+NX stack, with adjacent guards absent. The gate accepts
no caller-selected pointer. It recovers the exact running task incarnation and
trusted `shell_minimum` grant from kernel state, regardless of whether a future
request originates at a human shell, an AI client, or exec-hag.

Each 40-byte request prefix is copied once from the fixed mailbox before
architecture-neutral admission. Unknown operations, malformed structural
metadata, unauthorized rights, and oversized declarations are rejected before
payload bytes are read. The kernel-wide rite policy remains 4 KiB; this
one-page transport reserves the call prefix and therefore imposes a smaller
4,056-byte body cap. Only an admitted length is copied once into zero-initialized
kernel storage for operation-specific validation. Request, reply, and exit-body
layouts have compile-time checked offsets with no implicit padding, and every
reserved reply byte is explicitly zero.

The fixed nine-call sequence proves:

1. `bad_operation` for an unknown operation at sequence 1;
2. `denied` for a missing `console_write` right at sequence 2;
3. `denied` for a `display_update` overclaim against the shell grant at sequence
   3;
4. `limit_exceeded` for a 4,057-byte body above the transport cap at sequence 4;
5. `bad_payload` after copying and rejecting invalid UTF-8 at sequence 5;
6. an `ok` commit of 16 private printable bytes through `screen_console` at
   sequence 6;
7. `would_block` with no commit for empty `input_read` at sequence 7;
8. `bad_payload` for a copied exit body with nonzero reserved data at sequence 8;
   and
9. an `ok` terminal `task_exit` at sequence 9.

The first eight calls return through `iretq` with full reply validation. The
terminal call deliberately copies no reply and consumes a single-use saved
kernel continuation, so it reaches the restored kernel stack without returning
to the exited task. Only static operation classes, bounded status/sequence
metadata, and aggregate teardown facts reach serial; the private console body,
trace identities, registers, addresses, and arbitrary mailbox bytes do not.

The strict QEMU verifier accepted exactly 20 structured records: seven boot
records followed by the ready/task records, nine ordered outcomes, release, and
pass. Teardown restored all 65,116 initial free frames, zeroed active task,
circle, mapping, data-owned, and table-owned counts, retired the channel, wiped
the mailbox and transition stack, restored kernel CR3 and `CR0.TS`, and cleared
and released user and page-table storage. `make check`, every established QEMU
smoke gate, `make rite-smoke`, and normal-image exclusion passed.

Standing alone, the ADR 0019 image is a serialized, BSP-only proof with IF
clear and NMI masked. It does not exercise concurrent mailbox mutation,
preemption, SMP, general user-copy, nonempty input delivery, general blocking,
or composition with ADR 0018 supervision. ADR 0020 separately composes the
accepted mechanisms under another fixed feature, as described below. Neither
proof is a user-space shell or production rite/syscall surface. ADR 0019's
nonlocal completion remains valid only while abandoned Rust frames contain no
destructors, RAII guards, or lasting borrows.

## Accepted fixed supervised shell-vessel composition

ADR 0020 adds the removable `shell_vessel_test` image without changing the
normal kernel or `anomalous-abi`. It reuses the accepted 22-qword scheduled
context, allocator-owned task spaces and privilege stacks, fixed supervision
relationship, and architecture-neutral rite admission. Shared x86-64 transport
code now performs explicit little-endian prefix decode/reply encode, checked
one-page spans, volatile admitted-body copy, fixed exit decoding, and verified
scratch clearing for both rite proof images.

One persistent supervisor owns a fixed relationship across two destructive
shell-vessel incarnations. Each incarnation receives an exact task-bound
`shell_minimum` channel. Generation 1 validates one compiled human source,
derives a 17-byte private console body, commits `console_write` at sequence
one, observes empty `input_read` at sequence two, survives one real PIT
preemption, and reaches an exact contained `#UD`. Only after the supervisor
consumes that observation and the original kernel stack is restored does the
kernel retire the channel and clear and withdraw the faulted generation.

Generation 2 receives fresh task/circle identities and a fresh channel at
sequence one. Before issuing a rite, it must observe zeroed reused mailbox and
stack storage. Its machine-direct console call must equal the first
incarnation's operation, payload kind, declared right, bounds, body, and result
while using a distinct private trace. It observes the same empty-input result,
then submits a zero-status `task_exit` at sequence three.

Terminal exit is one bounded one-way transition: rite completion marks the
scheduled task exited, commits a `successful_exit` supervision publication,
wakes the exact blocked supervisor, consumes the observation, and makes the
feature gate select the supervisor's saved frame. The success reply is
constructed but never copied; the exited vessel is never an `iretq` target.
Both Rust dispatchers return before assembly pivots to another saved frame, so
no live Rust automatic or borrow is abandoned by the frame selection.

The strict QEMU verifier accepted debug-exit status 33 and exactly 38 schema-1
information records: eight boot records plus 30 fixed composition records. It
observed six dispatches, PIT tick 11, one contained ELF-derived fault, one
restart, five rites with fresh sequence after replacement, two equal private
console commits, two empty-input results, one terminal exit, two observations,
and full supervisor continuity. Teardown retired both channels, wiped scratch,
transport, user, privilege-stack, transition-stack, and page-table storage,
returned all active counts to zero, and restored all 65,102 initial free
frames.

Serial contains no console source or output, trace, mailbox byte, register,
root, address, or private marker. The verifier rejects exact lifecycle,
terminal-return, disclosure, and release mutations. Normal-image inspection
excludes the feature namespace, gate/runtime and fixture symbols, private
source/output probes including split instruction fragments, and every private
marker.

This demonstrates only that the fixed mechanisms compose on one BSP with NMI
masked and no hostile concurrent mailbox writer. It does not provide nonempty
input, general blocking, arbitrary launch, a shell parser/session, Canticle,
authenticated clients, production scheduling or supervision, SMP, APIC,
DMA/IOMMU isolation, or a stable normal-image ABI. Failures outside the exact
success path remain terminal and may quarantine ownership rather than attempt
unsafe rollback.

The kernel and native rite boundary remain x86-64. IA-32 execution and 32-bit
Win32 ABI details terminate in a future user-space compatibility veil, where
foreign pointers, handles, PE/NT structures, object conventions, and calling
conventions must be validated before translation to fixed-width native rites.
The composition adds no IA-32 kernel target and no second foreign control
surface.

## Accepted bounded user-space display presentation

[ADR 0024](../adr/0024-prove-bounded-user-space-display-presentation.md)
accepts a removable `display_vessel_test` image. It reuses the native x86-64
Rite gate and fixed allocator-owned task space, but has its own closed task,
channel, presenter, transcript, fixture, and normal-image exclusion boundary.
The ring-3 task holds exactly `display_update | task_exit` and maps one RX code
page, one RW+NX mailbox page, and one RW+NX stack page.

The version-1 display body is an exact 40-byte little-endian header followed by
tightly packed canonical RGB8 pixels. The one-page mailbox limits a patch to
4,016 pixel bytes. Rite admission validates the exact operation, trusted task
incarnation, grant, sequence, payload class, and body extent before architecture
code copies the complete body once into fixed kernel scratch. Header, row,
pixel, rectangle, output, and transport bounds are revalidated against the
kernel-owned presenter before its first framebuffer write.

The presenter is move-only and retains the unique validated framebuffer
aperture. Ring 3 receives no address, pitch, channel mask, raw mapping, or
direct write authority. An explicit walk of the feature task's root requires
the complete identity-mapped aperture to remain supervisor-only. A malformed,
denied, stale, out-of-bounds, or failed-copy call therefore changes no
framebuffer byte; an accepted call converts RGB8 through the validated channel
masks, writes exactly one rectangle, and publishes success only afterward.

At implementation checkpoint `b6546cc`, the strict QEMU proof accepted exactly
16 records: seven boot records and nine display records. Its fixed vessel made
772 calls consisting of denied-authority, malformed-flags, and out-of-bounds
rejections; 768 full-width row commits; and one terminal exit. Each rejection
preserved every byte of the canary-backed aperture. Terminal exit did not
return to the task, and teardown cleared copied scratch and withdrew the exact
channel, mappings, root, user and page-table storage, and transition state
before restoring allocator and object accounting.

After teardown, the host captured the retained strict-P6 1024x768 frame. An
independent public-fixture oracle matched all 786,432 RGB pixels. `make
display-vessel-smoke` reproduces that automated evidence; `make display-demo`
boots the same inspected image and leaves the completed frame visible for
human review. The display feature, gate, fixture, marker, helper symbols, and
payload remain absent from the normal image.

This is a serialized BSP-only mechanism proof with IF clear and NMI masked. It
does not define a compositor, window or surface protocol, authoritative
semantic scene, focus, hit testing, input routing, display manager, general
task launch, GPU path, page flipping, vsync, multi-monitor support, or stable
graphics ABI. The task root still borrows the kernel's supervisor-only low
identity branch. The U/S path denies CPL3 framebuffer access, but the proof
does not claim KPTI or a Meltdown-class speculative-isolation boundary.
Pixels remain a presentation projection; a later user-space decision must
derive both accessible human rendering and machine-readable state from one
canonical committed scene revision.

## Accepted bounded native semantic-surface transport

[ADR 0028](../adr/0028-prove-bounded-native-semantic-surface-transport.md)
accepts one removable x86-64 `semantic_surface_transport_test` proof. Two
separate fixed ring-3 tasks receive only disjoint
`semantic_surface_submit | task_exit` and `semantic_surface_read | task_exit`
grants. The source submits one exact versioned 528-byte semantic-surface body
to a kernel-owned one-slot endpoint; the consumer later receives that copied
body and independently validates the complete fixed profile. Neither public
surface, scene, revision, renderer, or attempt value confers authority: the
kernel binds the exact live task incarnations and endpoint state, and performs
only structural wire and correlation validation.

The endpoint copy is transactional and failure-atomic. Submit copies before
parse and commits only after complete validation; read stages the full reply,
publishes its header last, and consumes the slot only after the destination
copy is ready. Malformed, denied, stale, full, empty, and failed-copy paths
leave the relevant owner unchanged. Once the one valid submit commits, the
source can exit and have its channel, mailbox, mappings, task storage, and
address space withdrawn before the consumer delivery; the endpoint owns the
independent body until that delivery. Terminal teardown clears feature-owned
scratch, endpoint, channel, task, mapping, scheduler, and allocator state and
restores exact accounting.

The strict QEMU transcript exposes only fixed counts and invariant booleans;
it is closed-world, redacted, and rejects private content or correlating fields.
The ordinary image excludes the feature, payloads, generated body, and private
fixture probes. This is not a renderer, compositor, display path, presentation
or scanout proof, semantic publication mechanism, or stable IPC/graphics ABI.

## Accepted bounded native semantic render-and-present composition

[ADR 0029](../adr/0029-prove-bounded-native-semantic-render-and-present-composition.md)
accepts a separate highest-precedence
`semantic_surface_presentation_test` image. Its one-page source has only
`semantic_surface_submit | task_exit`; its three-page renderer has only
`semantic_surface_read | display_update | task_exit`. The source queues the
exact 528-byte ordinal-61 body derived from a retained live ADR 0027
transaction, exits, and is completely withdrawn before the renderer receives
the independent endpoint copy. Ring 3 maps neither the framebuffer nor the
576-frame private stage.

The renderer copies the delivered body into owned target storage, validates
the complete profile, and rasterizes directly from its typed transport view.
The kernel accepts exactly 768 ascending 1024-pixel RGB8 rows. A move-only
delivery owner binds the endpoint, source and renderer incarnations, channel,
supervisor, scene and surface correlation, attempt, and stage generation. A
distinct completion-only supervision lifecycle consumes the exact successful
exit without manufacturing a fault or restart; acknowledgement requires a
complete delivery and stage. Source, renderer, supervisor, endpoint, channels,
scheduler entries, address spaces, mappings, scratch, mailboxes, stacks, and
images are then withdrawn or cleared while only the unbound completed-stage
owner survives.

From a safe kernel stack, the proof preflights one exclusive presenter and
complete source coverage before the first framebuffer mutation. Its initial
framebuffer pattern is a declared mutation baseline, not a visual rendering of
ADR 0027 revision 1. After the non-rejecting full-frame copy, the kernel reads
back every visible channel byte against the completed stage and every padding
byte against that baseline, then clears and releases the stage and restores
exact accounting.

At implementation checkpoint `2f453e7`, the successful QEMU proof emitted
exactly seven feature records and completed 777 Rite calls, 775 body copies,
2,392,160 copied body bytes, and 768 row commits. It restored all 65,095
initial free frames with no active lease or owned data/table frame. The
2,359,312-byte strict-P6 capture matched an independent host oracle at all
786,432 pixels before the retained host transaction advanced from revision
and publication sequence 1/1 to 2/2.

The native run proves the success path only. Early exit, partial completion,
stale or foreign identity, wrong outcome, and wrong supervision phase are
excluded by the kernel's structural acknowledgement predicates and covered by
architecture-neutral component tests, not by a separate negative x86-64 run.
The feature provides no native retry, cross-process presentation authority,
scanout completion, stable ABI, compositor, window service, or normal-image
desktop. Host failure after framebuffer copy but before semantic publication
can still leave pixels at revision 2 while semantic readers observe revision
1; the feature has no rollback for that crash gap.

## Accepted host-only shell-session ingress model

[ADR 0021](../adr/0021-prove-bounded-private-shell-session-ingress.md)
accepts a separate `no_std`, allocation-free, unsafe-free host proof above the
kernel. The `anomalous-session` crate is not linked into the kernel or
`anomalous-abi`; it adds no gate, wire layout, QEMU feature, or normal-image
code. Its one-session model admits bounded semantic actions into a private
four-entry FIFO and couples exact prepare/commit waiting to enqueue-before-wake,
single-use acknowledgement, close tombstones, nonwrapping freshness identities,
and revision headroom for orderly retirement.

This host model treats origin as attribution, not authority. A trusted router
outside the crate must stamp the origin after establishing route policy, and
the embedding host must exclusively reserve the session-slot lineage. While the
owner remains live, the host must explicitly close it, deliver any terminal
notice, and obtain the exact acknowledgement before destruction. `Drop` only
performs a logical clear; it cannot close the protocol or wake a waiter, and
ordinary stores do not establish secure erasure. Abrupt owner loss can discard
accepted input or orphan a waiter.

Private owners are move-only and expose content-derived values only through
deliberately named local accessors. Content-free errors and observations carry
no private correlator or exact length, but they authorize no emission,
transport, or retention; cadence, volume, external timestamps, resource
pressure, and denial of service remain observable.

ADR 0022 now closes that gap only for one removable x86-64 feature proof. It
adds a distinct versioned semantic-session reply, copy-before-consume,
owned-pending correlation, exact nonterminal wake, and route-close
interruption without changing the ordinary physical `input_read` rite. It is
not a general wait/wake facility, production transport, public publisher, or
normal-image session path. ADR 0023 separately accepts a host-only router proof
that consumes an external authentication assertion; neither record provides
credential authentication, focus policy or secure attention, secret entry,
history, parsing, evaluation, Canticle, or a stable ABI.

## Deliberate limits

The kernel foundation, including its bounded feature proofs, does not:

- add a heap or general allocator/mapping API;
- replace Threshold's transition GDT or cover faults before Rust kernel entry;
- unmask NMI;
- discover ACPI/MADT topology or support APIC, MSI, SMP interrupt routing,
  calibrated clocks, deadlines, production preemption policy, general blocking
  waits, production nonempty input delivery, or general input-ready task wake;
- provide guard pages for the fixed exception stacks;
- symbolize faults in the kernel;
- provide general exception recovery;
- create general unprivileged execution, multitask circle, or vessel
  facilities; or
- establish authenticated principals, a stable task/sigil/IPC or graphics ABI,
  a semantic desktop, or DMA isolation.

The fixed experiments are not a general task model, production scheduler,
syscall surface, IPC transport, loader, or production supervisor/vessel runtime.
ADRs 0014, 0015, 0018, 0019, and 0020 establish reusable bounded lifecycle,
authority, copied-message, saved-context, switching, supervision, rite, and
fixed composition mechanisms plus narrow hardware-boundary evidence; later
decisions must still define general circles, task admission and policy,
user-copy behavior, production task-local waiting and exact wake, production
supervision, and stable control semantics. ADR 0021 adds host-only session
state-machine evidence; ADR 0022 adds only one removable native
copy/consume-and-wake proof. ADR 0024 adds only one removable native
display-presentation proof beneath a future semantic compositor. ADR 0028 adds
only one copied and versioned source-to-consumer semantic-surface transport.
ADR 0029 composes one exact live successor through rendering, staging,
framebuffer verification, and host publication, but only as a removable
single-success conformance bridge. Neither record establishes a general native
IPC or window service, production compositor, retry policy, scanout proof,
crash-atomic pixel/semantic commit, or normal-image desktop.
