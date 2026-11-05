# Capability roadmap

Anomalous advances when a capability has evidence, not when a date arrives.
Each milestone must preserve boot diagnostics, document its invariants and
failure modes, and expose equivalent truth to human and machine clients.

## Current selected sequence — kernel foundations, then Linux Antimatter

**Current product Milestone 1 is the existing Linux Antimatter application
running, rendering and receiving input inside Anomalous.**
[ADR 0073](adr/0073-run-linux-antimatter-through-the-linux-veil.md) supersedes the
native frontend port expectation. Start with ordinary Linux executables and
their unchanged interpreter/libraries; progress to Electron and the real
Antimatter bundle. Native proofs remain the ownership/service baseline; Windows
and macOS follow Linux. A fixed ABI fixture does not meet the application
milestone. The numbered milestones farther below retain historical names.

[ADR 0074](adr/0074-plan-linux-images-and-initial-stacks.md) established host
image and startup planning; accepted
[ADR 0095](adr/0095-prepare-and-cancel-authenticated-source-mappings.md) and
[ADR 0096](adr/0096-retain-publish-and-withdraw-blocked-caller-mappings.md)
qualified detached preparation and retained inactive publication. Accepted
[ADR 0097](adr/0097-resume-a-caller-with-a-retained-source-mapping.md) qualifies
the first read-only libc mapping return at 4 GiB,
actual interpreter resumption and capture of its next pending request. Fresh
emitted reviews, four TCG/KVM boots with eight lifetimes, independent transcript
verification, 22 malformed-trace checks, six provisioning refusals and the
thirty-command regression matrix pass. Both exact additional workspace runtime
checks and the final audit pass. The
[Mapping Resume Log](../notebook/Development/2026-10-18%20Mapping%20Resume%20Log.md)
records evidence, substantial development-image differences and retained failures.

The caller is suspended at sequence 12 after eleven Linux returns. The next
request is `mmap(0x100028000, 0x198000, 5, 0x812, 3, 0x28000)`, requesting a
fixed private read/execute mapping. Select the next bounded userspace Linux
policy from that observation while preserving retained ownership and fault
cleanup. Further mapping/protection, dynamic linking, main `true` entry, Linux
exit and Electron/Antimatter execution remain open. Candidate-leaf Accessed
evidence is separate from proof that the interpreter resumed. Foreign syscall
and ELF policy remain in userspace; Windows and macOS follow the Linux baseline.

The desktop launcher and simplified bottom panel are qualified for the five
available built-ins, with exact QEMU interaction frames and a matching
VirtualBox boot/render comparison. The
[Workspace Launcher Log](../notebook/Development/2026-10-04%20Workspace%20Launcher%20Log.md)
records the evidence. This does not imply general artifact loading or an
Antimatter port. Reliquary uses `.re`; accepted
[ADR 0061](adr/0061-select-re-extension-and-path-policy.md) also requires native
path handling without a fixed global length/depth ceiling.

Accepted [ADR 0062](adr/0062-design-native-object-filesystem.md) selects the
[native filesystem direction](architecture/filesystem.md), extending that rule
to individual names. Stable objects and immutable versions separate identity
from paths; copy-on-write transactions, snapshots, independently restorable
backups and mirror repair have distinct proof gates. No general filesystem is
implemented. Proposed [ADR 0063](adr/0063-prove-native-durable-document-storage.md)
now connects ADR 0060's portable model to a scoped disposable data disk and
qualifies exact recovery after complete service loss and cold QEMU restart.
Native device/service fault cuts, lost committed replies and newer unsaved
edits remain the next durability cases. A full filesystem is not required
before the first Antimatter application proof.

The earlier native-first sequence established the current baselines. Antimatter is the official
editor, Ny is the official language direction for Anomalous and HAG, and HAG is
the compute runtime intended to replace Vulkan. Accepted
[ADR 0045](adr/0045-select-native-antimatter-ny-and-hag-direction.md) records
the direction while keeping implementation gates open. The native ABI baseline
and the selected bounded ELF loading comparison are qualified. Explicit Linux
startup and a bounded record-device syscall personality are qualified in
ADR 0052. ADR 0053 implements that foreign policy in an independent userspace
veil; qualification passed 39 matrix jobs and two restorations. ADR 0054 now
qualifies a shared two-caller veil through 41 qualification checks and two restorations.
Proposed [ADR 0055](adr/0055-prove-native-startup-and-private-userspace-arenas.md) implements copied native
startup and private arenas. All 43 checks and two image restorations passed;
receipts are in the [Native Runtime Log](../notebook/Development/2026-10-04%20Native%20Runtime%20Log.md).
Proposed [ADR 0056](adr/0056-prove-task-bound-native-document-open-and-save.md) implements bounded native RAM
document open/save and close/reopen in a surviving service. Bounded native
qualification passed; the
[Native Document Log](../notebook/Development/2026-10-04%20Native%20Document%20Log.md)
records the evidence.
Proposed [ADR 0057](adr/0057-prove-elf-loading-of-the-native-document-contract.md)
qualifies the same bounded contract through ELF while preserving native client
bytes, the native personality and existing loader bounds. The
[ELF Document Log](../notebook/Development/2026-10-04%20ELF%20Document%20Log.md)
records the completed qualification. Proposed
[ADR 0058](adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md)
qualifies native editor replacement while retaining the RAM service. All 50
qualification checks and two development-image restorations passed; the
[Native Relaunch Log](../notebook/Development/2026-10-04%20Native%20Relaunch%20Log.md)
records the completed evidence.
Proposed [ADR 0059](adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md)
implements the same retained-service replacement through the userspace ELF loader
against the native baseline. All 52 qualification checks and two
development-image restorations passed; the
[ELF Relaunch Log](../notebook/Development/2026-10-04%20ELF%20Relaunch%20Log.md)
records the completed bounded comparison. The separate native disk proof now
qualifies normal save and cold recovery; its remaining failure cases and
interactive frontend integration are open. The Linux Antimatter copy
outside the VM remains the behavioral reference. Windows and macOS comparisons come later,
each with its own execution/service personality.

The [direction log](../notebook/Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
records the sibling-project audit and the later Linux application decision.
Ny and HAG retain separate target/runtime work; Antimatter's existing Linux
frontend is the selected application. Software rendering can precede HAG
acceleration; verified Rust remains authoritative until qualified replacements.

The completed foundation checkpoints remain the starting evidence:
Proposed [ADR 0039](adr/0039-prove-bounded-sustained-native-dispatch.md) selects a
separate sustained-dispatch proof: 64 real PIT switches between two private
GPR-only tasks, one exact known fault, and 16 further timer ticks with measured
survivor progress and no artificial redispatch. Final TCG and KVM runs passed
with exact restoration of 65,112 free frames. All 23 validation targets passed,
including the full ordinary regression matrix and separate KVM dispatcher and
workspace repeats. An isolated VirtualBox attempt was blocked by host KVM
ownership of AMD-V and cleaned up. The workspace image is byte-identical to
the preceding graphics checkpoint.

The memory-planning checkpoint, proposed
[ADR 0041](adr/0041-plan-bounded-user-mappings-and-physical-access.md), adds inert
bounded user-mapping plans and typed physical/kernel virtual-window geometry.
Nineteen new tests validate complete ranges, permissions, exclusions, exact
branch budgets, executable gaps, and offset conversions. Focused checks and all
23 regression targets passed; normal, dispatcher, and workspace images remain
byte-identical to the preceding checkpoint. This prepares owned table construction
without changing the live mapper's low-address exclusion or activating new
translations. The [mapping log](../notebook/Development/2026-09-26%20User%20Mapping%20Plan%20Log.md)
separates arithmetic/planning evidence from the remaining native ownership proof.

The preceding increment, proposed
[ADR 0042](adr/0042-prove-inactive-owned-user-mapping-construction.md), now builds
and validates inactive multi-branch user trees using real allocator leases and a
temporary supervisor physical alias. Cleanup retains ownership on failure and
supports retries. Focused tests and all 24 regression targets passed, including
two-size native memory proofs under TCG and KVM. These user-only roots remain
inactive. The
[construction log](../notebook/Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md)
records the distinction between this owned stage and a running address space.

The preceding resident increment, proposed
[ADR 0043](adr/0043-prove-resident-mapping-activation.md), adds bounded explicit
kernel/stack residency and proves switching between two owned low-user roots.
The physical alias survives actual virtual/physical address collisions; exact
CPL0 read-only, guard and NX faults resume only their armed probes. Bootstrap
restoration precedes complete cleanup. The
[activation log](../notebook/Development/2026-09-26%20Resident%20Mapping%20Activation%20Log.md)
records all 24 passed regression targets, the two-size TCG/KVM memory proof, and
matching normal/dispatcher/workspace images at that checkpoint. Durable kernel
placement remains a separate gate.

The preceding increment, proposed
[ADR 0044](adr/0044-prove-owned-low-address-user-execution.md), enters two real
GPR-only ring-3 tasks over those retained owned mappings. It admits initialized
code and guarded stack bounds, resumes four exact reports at noninitial stack
pointers, contains B's supervisor-read fault, and lets A continue to exit. All
28 frames, eight leases, task/circle identities, and transition state are
restored. Shared exception snapshots now use actual hardware-saved RSP/SS for
CPL0/CPL3/IST entry. All 24 validation targets passed, including both RAM sizes
under TCG and KVM, with 240 frozen source/configuration files unchanged. The
[user execution log](../notebook/Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records the exact 23-record native evidence and changed runtime-image hashes
from the shared diagnostic correction. This closes a bounded user-entry experiment, not a general native launcher.

The preceding increment, proposed
[ADR 0046](adr/0046-prove-retained-prepared-launch-ownership.md), retains launch
ownership for that same three-region GPR-only profile. Preparation builds and
verifies mappings/payloads and attaches circle resources without publishing a
task. Explicit commit creates the ready task; `cancel_unstarted` makes an
unstarted ready task terminal before retryable mapping cleanup. Thirteen native
failure/cancellation cases restore ownership before the original two users are
prepared, committed, and executed through this owner. Input stays inert and
format-neutral; separately supplied fixture identity is not authority derived
from an artifact.

Four task-transition tests, eight owner tests, seven memory-verifier tests,
freestanding Clippy, and 96-MiB/320-MiB TCG/KVM memory proofs passed. All 24
regression targets passed with 254 frozen non-documentation files unchanged.
Normal and workspace images match the recovery checkpoint. The
[Prepared Launch Ownership Log](../notebook/Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records validation. This is retained fixture ownership, not a general launcher
or application ABI.

The preceding increment, proposed
[ADR 0047](adr/0047-prove-sustained-dispatch-over-retained-launches.md), composes
two retained launches with sustained scheduling over owned low-address roots.
The aggregate keeps 36 frames (18 data, 18 tables), ten bound leases, and two
16 KiB supervisor privilege stacks reached through the high alias. Partial
scheduler admission rolls back without changing survivor order or sequence
cursors; two failed stack clears retain ownership through cleanup retries.
Native users complete 64 preemptions, one exact UD2 fault, and 16 survivor ticks
with nested RSP at stack top minus 24 bytes. The faulted owner stays retained
until both tasks are terminal. Mapping revalidation failures remain retained
fail-closed, distinct from retryable clear/release failures.

Six scheduler tests, four task tests, six verifier groups, freestanding feature
Clippy, and 96-MiB/320-MiB TCG/KVM proofs passed. All 26 regression targets
qualified across documented phases in 1,491.42 seconds of selected target time.
The final 258-file freeze remained unchanged after a narrow display-capture
harness correction; normal/workspace images match recovery. The
[Owned Sustained Dispatch Log](../notebook/Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
records final evidence and the original interrupted-run diagnostic limit.
Use `make owned-dispatcher-smoke` for that separate two-size proof; the
previous dispatcher and memory proofs remain regression targets.

The preceding proposed
[ADR 0048](adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
implements an internal x87/SSE2 owner, bounded 4096-byte current-task copies,
and selected #DE/#UD/#GP/#PF containment with last-current fault cleanup.
Its separate fixture completed eight tasks across four scenarios, 64 preemptions
and 80 timer ticks per pair, and 20 successful/40 rejected copies per VM.
All 28 targets passed on 265 unchanged frozen source files, including the
96-MiB/320-MiB TCG Intel-baseline and actual AMD-host KVM proofs. Normal and
workspace images remain byte-identical to the preceding checkpoint. The
[Application Profile Log](../notebook/Development/2026-10-03%20Application%20Profile%20Log.md)
records final evidence and limits.
TLS, AVX/XSAVE, #MF/#XM recovery and full untrusted-program admission remain
outside this internal profile.

The preceding proposed
[ADR 0049](adr/0049-qualify-a-bounded-freestanding-ny-object.md)
qualifies one pinned Ny object and inspected startup/runtime shim. It proves
initialized-data and zero-fill evidence, 50,000,000 integer/f64 iterations,
timer returns inside generated code, two failure paths with successful peers,
and retained cleanup. All 30 targets passed on 272 unchanged frozen files,
including two-size TCG/AMD-host KVM proofs and unchanged normal/workspace images.
The [Ny Object Log](../notebook/Development/2026-10-03%20Ny%20Object%20Log.md)
records final evidence and limits.
It does not port the hosted runtime or establish a general compiler target.

Proposed [ADR 0050](adr/0050-prove-a-bounded-native-application-contract.md)
implements the preceding bounded Rust application/service prototype. Its real
Antimatter selection, theme and undo trace crosses two copied task-bound
channels, with independent software rendering, terminal revocation and deferred
presentation. All 33 matrix jobs passed on 291 unchanged frozen files, including
two-size TCG/KVM and capture proofs. The
[Native Application Log](../notebook/Development/2026-10-03%20Native%20Application%20Log.md)
records the reference harness, implementation and remaining limits.

The completed checkpoint under proposed
[ADR 0051](adr/0051-prove-userspace-elf-loading-against-native.md) implements a
bounded ELF loader in an independent ring-3 vessel. It parses the compact file,
submits a copied inert plan, and retires before native/ELF application routes
run. Exact manifest admission, initialized RW/BSS handling, service behavior,
surface hashes, pixels and cleanup form the comparison. Both routes retain the
native `int 0xf4` personality. All 35 matrix jobs passed on 2026-10-04 with 302
frozen source files unchanged, including two-size TCG/KVM proofs and captures.
ADR 0051 remains Proposed. The
[ELF Application Log](../notebook/Development/2026-10-03%20ELF%20Application%20Log.md)
records the evidence; this does not establish Linux or Electron compatibility.

The preceding qualified [ADR 0052](adr/0052-prove-linux-startup-and-record-device-syscalls.md)
implements a Linux startup/SYSCALL producer paired with that native service.
The same editor trace, task-owned nonblocking record devices, atomic input
consumption/poll copies, fault cleanup and restored entry controls passed
qualification on 2026-10-04. All 37 matrix jobs and two restoration jobs passed
on 313 unchanged frozen files, including two-size TCG/KVM proofs and captures.
The [Linux Application Log](../notebook/Development/2026-10-04%20Linux%20Application%20Log.md)
records the evidence. That feature-private proof controller is not general
Linux or Electron support.

The preceding proposed [ADR 0053](adr/0053-prove-a-supervised-userspace-syscall-veil.md)
implements a separate userspace policy vessel beside the unchanged producer and
native service. Sealed blocked-call ownership, inactive-caller checked copies,
inert staging and reply preflight keep publication inside native authority.
Normal exchange, queued producer fault and staged veil fault are the three
qualification scenarios. All 39 matrix jobs and two restorations passed; evidence is in the
[Userspace Veil Log](../notebook/Development/2026-10-04%20Userspace%20Veil%20Log.md);
fixed startup remains a controller fixture.

The preceding proposed [ADR 0054](adr/0054-prove-a-shared-multi-caller-userspace-veil.md) qualifies two independent Linux editor
producers and two native services sharing one supervised veil. Exact sealed
membership, per-caller routes and copied-memory isolation preserve the native
baseline. A caller fault leaves its peer progressing; a veil fault revokes both
dependents and discards its staged effect while both native services survive.
All 41 qualification checks and two image restorations passed, including TCG/Intel and
KVM/AMD at both guest sizes with capture comparison. All twelve preceding
embedded artifact blobs remain unchanged and no OOM occurred. The
[Shared Veil Log](../notebook/Development/2026-10-04%20Shared%20Veil%20Log.md) records the frozen-source and compiled-code evidence.
This is a fixed two-member serial service, not full Antimatter/Electron or a
general process/runtime service.

The native ABI/artifact format is not done. Experimental rite prefixes are
implemented, while canonical semantic invocation encoding, a versioned general
launch contract, and Reliquary execution framing remain open. The
[execution-readiness audit](architecture/execution-readiness.md) records these
boundaries and the source evidence. The next gates are:

1. Preserve ADR 0050's qualified native baseline: bounded reference-editor state behavior,
   retained ownership, exact copied frames and exit/fault cleanup. This bounded
   proof uses Rust; Ny's broader frontend/runtime requirements
   remain independently gated.
2. Preserve ADR 0055's qualified startup/arena profile and earlier ABI baselines.
   Preserve ADR 0056's qualified RAM document service, exact snapshot
   acknowledgements and unknown-outcome handling, together with ADR 0057's
   qualified bounded ELF comparison. Close/reopen keeps bytes only while the
   same service lives. Preserve ADR 0058's qualified native editor replacement
   with the service retained and ADR 0059's qualified bounded comparison of the
   same cases through the userspace ELF loader. Preserve native personality and
   independent document, lifecycle and pixel checks on both routes. Preserve
   ADR 0063's native disk save and cold-restart proof and subsequent qualified
   failure/recovery cases. Later add ADR 0062's chunked
   objects/names, transactions, snapshots, backups and mirrors
   through separate evidence. Live growth,
   threads/TLS, asynchronous events, queues and service restart retain independent
   gates. ELF parsing confers no execution authority.
3. Preserve the qualified authenticated mapping prepare/cancel boundary, then
   qualify combined mapping ownership, publication, withdrawal and exact Linux
   replies. Follow the unchanged interpreter through the remaining mapping,
   protection, file and TLS/libc startup services to a real small-program exit.
   Then qualify the process, file, synchronization, signal and display services
   needed by the selected Electron build. Preserve the native baseline.
4. Launch the existing Linux Antimatter bundle and compare its actual interface,
   input and open/edit/save/relaunch behavior with the same host build. Windows
   and macOS, Reliquary framing, inference and HAG acceleration follow separately.

Permanent kernel placement, unsupported CPU/fault classes, and migration of
legacy runtime owners remain separate gates. Rich parsers and foreign-platform
policy stay outside the kernel. A passing
dispatcher experiment, a parsed ELF, or a visible native graphics buffer alone
does not establish arbitrary application execution or complete compatibility.

The parallel [identity and authority design](architecture/identity-and-authority.md),
under proposed [ADR 0040](adr/0040-separate-entity-identity-authentication-and-authority.md),
separates entity identity, passwordless authenticators, live task/session
bindings, and scoped administrative powers. Future biometrics perform local
user verification for protected keys. Authentication providers and policy stay
outside the kernel; a real credential-to-task binding, revocation at effect
commit, and durable recovery still require implementation and evidence. This
design does not replace the address-space and launch prerequisites above.

## Milestone 0 — Threshold

**Status: implemented seed**

- Pin a reproducible Rust toolchain.
- Build an Anomalous-owned BIOS boot path without GRUB.
- Enter a `no_std` Rust kernel in x86-64 long mode.
- Hand off a versioned memory map and framebuffer description.
- Emit structured COM1 boot events and render an accessible neumorphic surface.
- Prove the path with an automated QEMU smoke test and a VirtualBox workflow.

Gate: a clean checkout can build a 16 MiB disk and report `boot.ready`.

## Milestone 1 — Kernel foundations

**Bring-up instrument implemented:** normal images now enter a bounded,
allocation-free bootstrap shell after `boot.ready`. It consumes interrupt-fed
physical PS/2 transitions through a separate provisional US ASCII adapter,
provides framebuffer and VGA-text consoles, five read-only local commands, safe
command-class telemetry, host parser/editor tests, and injected-keyboard QEMU
and VirtualBox evidence. This ring-0 loop is neither Canticle nor a security
boundary; it exists to shorten development feedback until an unprivileged shell
vessel is possible.

**Diagnostic slice implemented:** the x86-64 kernel now owns its GDT and TSS,
provides dedicated double-fault and NMI stacks, installs gates for all CPU
exception vectors, and emits structured fatal attribution. A test-only image
proves exact invalid-opcode delivery through this path. NMI and maskable
interrupts initially remain disabled until their independently tested paths are
ready.

**Fixed isolation experiment implemented:** ADR 0011 accepts a feature-only,
three-generation x86-64 proof using a private CR3. QEMU and VirtualBox enter
ring 3, produce exact faults for a present supervisor-only read and a present
NX stack fetch, clear private state with verified volatile teardown, and launch
a final clean generation. The strict `circle-smoke` trace, normal-image
exclusion, and existing regressions pass. This does not satisfy the milestone-2
requirements for general circles, vessels, tasks, scheduling, syscalls, IPC,
allocation, or supervision.

**Bounded memory ownership slice implemented and accepted:** ADR 0012 requires
a complete version-2 firmware map, copies and conservatively normalizes it into
a fixed frame ledger, reserves bootstrap, kernel, and display memory, and
grants allocator- and generation-bound data or page-table leases with verified
clearing. Feature-only QEMU proofs at 96 MiB and 320 MiB and a dedicated
512 MiB VirtualBox run map distinct RW+NX frames at one virtual address across
an A-to-B-to-A switch, reject overlap, duplicate binding, and active-root
destruction, restore CR3, withdraw both mappings, and prove zeroed same-frame
reuse with exact free-count restoration. At that checkpoint, the normal
VirtualBox image was restored and observed running at the bootstrap shell. This
is still a BSP-only, fixed-capacity ownership proof below four GiB, not a heap or
general process isolation.

**Bounded interrupt, time, and input slice implemented and accepted:** ADR 0013
adds BSP-only returning IRQ entry, a replaceable legacy PC PIC/PIT/8042 backend,
an immutable boot-time generation with same-timebase monotonic ticks, fixed
typed records with exact loss, and physical key transitions separate from
layout. Loss contains IRQ1 until bounded 8042/device reinitialization advances
the source generation and restarts its sequence. QEMU and VirtualBox both prove
real timer and keyboard return, 24 ordered private transitions, deterministic
loss and resynchronization, final masking, and privacy-safe diagnostics. The
normal shell now idles with `hlt` and drains the interrupt-fed input channel.
COM1 remains output-only, while AI control remains reserved for future
authenticated IPC and versioned invocations rather than synthetic input.

- Validate CPU features and firmware input before use.
- Establish physical and virtual memory management, a diagnostic exception path,
  IDT, APIC timer, symbolized panic records, and deterministic test entry points;
  then safely unmask NMI.
- Split Threshold into testable storage, platform, display, verification, and
  architecture modules; add a UEFI frontend that produces the same `boot_omen`.
- Define boot recovery and a measured/verified artifact path.

Gate: malformed boot input fails safely, exceptions are attributable, and both
BIOS and UEFI frontends pass the same handoff conformance suite.

## Milestone 2 — Circles, sigils, and vessels

**Bounded attributable control slice implemented and accepted:** ADR 0014 adds
architecture-neutral fixed tables for 16 circles, 16 tasks, four endpoints,
16 sigil records, four task-local bindings per task, and four revocation
lineages. Every live identity is generational and nonwrapping. The serialized
BSP proof permits only one running task and gives each circle one
allocator-backed root with exactly three user mappings: RX code, an RW+NX fixed
mailbox, and an RW+NX stack, with adjacent guards verified absent.

A feature-only DPL-3 gate binds exact ring-3 state to the kernel-selected task.
Task-local `SIG1` references resolve through fresh task, holder, circle,
endpoint, right, and lineage checks before composite authorized IPC can run.
The two-slot kernel FIFO applies explicit backpressure, copies and stamps
nonwrapping sequences plus kernel task/circle/principal/origin provenance, and
commits receiver mailbox copy-out before consuming a message. Teardown restores
the kernel CR3 and pre-entry `CR0.TS` state, clears saved transition context,
withdraws mappings and frames in dependency order, and verifies user, queue,
table, and privilege-stack storage clear before reuse.

Byte-identical human and machine producer fixtures pass through the same gate,
authorization, FIFO, return, and teardown path while retaining distinct
kernel-selected synthetic provenance. They are not authenticated principals.
Acceptance included `make check` with 193 kernel tests and every QEMU
regression. The QEMU control proof accepted exactly 28 records and restored
65,114 free frames; the exact VirtualBox VM `Anomalous Dev [Threshold]`
accepted the same 28 records and restored 130,667. Strict normal-image exclusion
passed before the ordinary image was restored, observed running, and reported
130,683 free frames.

**Bounded scheduling and fault-containment slice implemented and accepted:**
ADR 0015 adds a fixed round-robin scheduler table and binds exactly two
allocator-backed task contexts to distinct private roots and 16 KiB privilege
stacks. Two real PIT IRQ0 deliveries preempt each task once. The faulting task
then resumes and executes one exact `ud2`; only the complete expected fault
predicate terminates that task, after which the peer resumes and completes.
QEMU and the exact VirtualBox VM each passed the strict 25-record lifecycle,
restoring 65,112 and 130,665 initial free frames respectively. Every established
QEMU regression and normal-image exclusion passed before the ordinary
VirtualBox image was restored and left running at `bootstrap_shell.ready` with
130,683 free frames.

**Bounded vessel supervision and restart implemented and accepted:** ADR 0018
adds a fixed-capacity supervision relationship, one persistent supervisor task,
and one restartable vessel generation at a time. The feature-only x86-64 proof
blocks and resumes the supervisor around exact kernel-owned observations,
withdraws a faulted first generation at a safe kernel-stack checkpoint, creates
a cleared second generation, consumes its successful-exit observation, and
restores all scheduler, supervision, task, circle, mapping, and allocator
accounting. It is not a service loader, package lifecycle, general vessel
manager, or production restart policy.

**Bounded native rite boundary implemented and accepted:** ADR 0019 defines
client-neutral, fixed-width call/reply prefixes and a task-bound admission core,
then proves them through a removable x86-64 `rite_test` image. One fixed mailbox
carries nine ordered ring-3 calls: six payload-free rejection paths, a 16-byte
`console_write` commit, empty `input_read` as `would_block`, and terminal
`task_exit` without reply copy or `iretq`. The one-page transport caps the body
at 4,056 bytes beneath the independent 4 KiB kernel policy.

The strict QEMU verifier accepted exactly 20 records, restored all 65,116 initial
free frames and zero active/data/table accounting, and excluded the private
payload from serial. `make check`, every established QEMU regression,
`make rite-smoke`, and normal-image exclusion passed. Standing alone, the proof
is one BSP-only shell-task fixture modeling a future vessel; it does not move
the production shell out of ring 0 or create a normal-image or stable syscall
ABI.

**Fixed supervised shell-vessel composition implemented and accepted:** ADR
0020 composes the accepted scheduler, one-restart supervision relationship, and
native rite boundary in a separate removable x86-64 image. Generation 1 projects
one exact human source to a private 17-byte console rite, observes empty input,
is preempted by the PIT, and faults at one exact `#UD`. After safe destructive
withdrawal, generation 2 starts a fresh task-bound channel at sequence one,
constructs the same console semantics directly, observes the same empty-input
result, and exits through a terminal rite that selects the awakened supervisor
without copying a reply or returning to the exited vessel.

The strict QEMU verifier accepted debug-exit status 33 and exactly 38
information records, including five rites, PIT tick 11, one restart, two equal
console commits, two observations, and full reclamation of all 65,102 initial
free frames. Privacy/mutation verification, normal-image exclusion, every
established QEMU regression, host tests, and warning-denied x86-64 Clippy
passed. VirtualBox was not run for this acceptance.

This proves only the fixed low-level composition. It does not add Canticle,
nonempty user-space input or blocking, arbitrary vessel launch, a service
manager, authenticated semantic clients, production scheduler/supervision
policy, or a normal-image/stable ABI. The native target remains x86-64; IA-32
machine code and 32-bit Win32 ABI translation remain isolated user-space
compatibility-veil work.

**Bounded private shell-session ingress implemented and accepted:** ADR 0021
adds the host-only, `no_std` `anomalous-session` proof. One fixed session accepts
bounded semantic editing actions into private move-only storage, applies exact
FIFO backpressure, binds one consumer wait to nonwrapping session/consumer/route
freshness, closes without a lost wake, and permits replacement only after
retirement. Origin labels remain attribution rather than authentication, and
logical clearing is not a secure-erasure claim.

At implementation checkpoint `a2c72b2`, 51 unit tests and 56 compile-fail
documentation tests passed. `make check`, warning-denied host and x86-64 Clippy,
every established QEMU smoke target, and normal-image exclusion also passed.
VirtualBox was not run or required because the proof changes no boot or kernel
code. The crate is not linked into the kernel or rite transport and supplies no
Canticle syntax, authenticated router, native reply, general wait/wake
mechanism, or shell runtime by itself.

**Bounded native session input and exact wake implemented and accepted:** ADR
0022 adds one removable x86-64 feature proof for a versioned nonempty semantic
reply, owned pending read, copy-before-consume delivery, exact blocked-consumer
wake, route-close interruption, and teardown. Its publisher is a kernel fixture
rather than an authenticated route, and the accepted mechanism is neither a
normal-image session transport nor a general scheduler facility.

**Bounded user-space display presentation implemented and accepted:** ADR 0024
adds one removable x86-64 display vessel with only `display_update | task_exit`,
three fixed user pages, and no framebuffer mapping. The kernel copies and
validates exact versioned RGB8 patches before its exclusive presenter writes
the aperture. The strict QEMU proof rejects denied, malformed, and
out-of-bounds calls without changing any canary-backed framebuffer byte,
commits 768 full-width rows, exits and tears down, then matches all 786,432
captured pixels against an independent oracle. This is a presentation
mechanism beneath a future semantic desktop, not a compositor or normal-image
graphics runtime.

**Canonical semantic desktop scene accepted:** ADR 0025 accepts a separate
host-only, eight-node scene proof. One atomic revision derives an accessible
semantic render plan and a machine-readable projection together, with exact
action/focus parity and protected-label redaction across the complete 96-state
matrix. The proof passed 18 unit tests, 129 ownership/privacy compile-fail
tests, the complete configured host checks, every established QEMU regression,
and normal-image exclusion. It is not itself a renderer, compositor, window
protocol, input/focus policy, or deployed desktop service.

**Supervised semantic renderer implemented and accepted:** ADR 0026 composes
that accepted scene into native pixels through a bounded redacted capsule and
a separately linked 6,469-byte, two-page executable load image. One persistent
supervisor discards generation 1 after a real PIT preemption, 384 private rows,
and an exact `#UD`; a fresh generation stages 768 rows, exits terminally, and
alone authorizes one full-frame publication. The strict capture matches the
independent oracle at all 786,432 pixels, complete resource accounting is
restored, and ordinary images exclude the feature, glyph fingerprint, and
protected-source sentinels. This remains a fixed proof rather than a live scene
transport, compositor, window protocol, input/action path, or normal-image
desktop runtime.

**Transactional semantic-surface service implemented and accepted:** ADR 0027
adds the host-only, `no_std`, allocation-free, unsafe-free
`anomalous-desktop-service` proof around ADR 0025's fixed 1024x768, eight-node
scene profile. One complete committed revision remains public while at most one
complete successor occupies the service's queued, leased, ready, or presented
phase. Full capacity returns the candidate owner unchanged; renderer readiness
does not publish; and only an exact trusted presentation receipt commits the
new human, accessibility, and machine projections together.

The service gives one immutable redacted snapshot to one exact renderer
endpoint at a time. One controller-authorized replacement preserves that
snapshot under a fresh renderer generation and second delivery attempt while
old endpoints and progression owners remain stale. Its branded action/focus
facade exposes only the committed scene and makes revision-old handles stale at
publication without executing an effect. Host validation covers all 64 valid
initial states, all 32 invalid focus states, all 64-by-64 valid transitions,
strict one-slot backpressure through every occupied phase, directed restart and
retirement paths, equal-public-ID/different-brand denial, protected-value and
length noninterference, static errors, and move-only ownership constraints.

This acceptance changes no kernel, `anomalous-abi`, Rite, renderer image,
ADR 0026 capsule, boot path, or normal-image runtime. The presentation receipt
is deliberately a trusted host assertion rather than pixel or scanout proof,
and the service defines no wire format, native transport, compositor, window
lifecycle, input/focus policy, or stable graphics ABI.

**Bounded native semantic-surface transport implemented and accepted:** ADR
0028 adds the missing removable native wire proof at implementation checkpoint
`228a168`. One fixed ring-3 source with only
`semantic_surface_submit | task_exit` and one fixed ring-3 consumer with only
`semantic_surface_read | task_exit` exchange one versioned 528-byte body through
a kernel-owned one-slot copied endpoint. The kernel performs structural and
trusted-binding validation, while the consumer independently validates the
complete fixed profile. Submit and delivery are transactional and failure-atomic:
the source may retire before delivery because the endpoint owns its exact copied
body, and rejected or fallible paths leave ownership unchanged.

The proof records only redacted, closed-world lifecycle evidence and restores
all feature-owned channel, endpoint, scratch, task, mapping, scheduler, and
allocator state before reuse. It does not render, write a framebuffer, present,
scan out, or publish semantic state.

**Bounded native semantic render-and-present composition implemented and
accepted:** ADR 0029 composes one exact live ordinal-61 ADR 0027 successor
through ADR 0028's copied transport, a separately linked ring-3 renderer, one
576-frame private stage, complete native teardown, one framebuffer commit, an
independent host pixel oracle, and finally the retained host controls'
publication to revision and sequence 2/2. The source is 2,384 bytes/one page;
the renderer is 10,352 bytes/three pages. At implementation checkpoint
`2f453e7`, the successful QEMU run emitted exactly seven feature records,
completed 777 Rite calls, 775 body copies, 2,392,160 copied body bytes, and 768
ordered rows, and restored all 65,095 initial free frames.

The initial framebuffer is only a declared mutation baseline, not proven
revision-1 pixels. The QEMU run covers success; early exit and partial
completion rejection are enforced structurally and covered by component tests,
not a separate negative native run. The proof retains the crash gap between
framebuffer mutation and host publication and adds no native retry, scanout
guarantee, stable ABI, compositor, window service, or normal-image desktop.
The full configured checks and QEMU matrix, dedicated display, supervised
renderer, transport and presentation proofs, and final normal-image exclusion
passed. VirtualBox was not required because Threshold and the ordinary boot
path did not change.

This advances the milestone but does not complete it. The kernel, firmware, and
emulator remain trusted, while the writable and executable supervisor identity
alias remains a ring-0 bypass. NMI is masked and execution is single-task
BSP-only outside the fixed feature proofs. There is still no production
scheduler, general preemption or admission policy, process or vessel runtime,
stable syscall/IPC ABI, authenticated identity, semantic control plane, general
supervision policy, production authenticated session router, production
semantic-input path or general wait/wake transaction, DMA isolation, or IOMMU
policy. ADR 0023 accepts only a host model that consumes an external
authentication assertion; it does not authenticate credentials or deploy that
router as a runtime service. ADRs 0025 through 0029 now provide one fixed host
scene, a removable supervised renderer, a bounded host publication
transaction, one copied native delivery, and one measured end-to-end success
composition. They do not provide a deployed native semantic/window service,
cross-process presentation authority, crash recovery, native retry, a deployed
accessibility service, focus/input policy, window lifecycle, display familiar,
production compositor, or stable graphics ABI.

**Interactive bootstrap development image implemented:** Proposed ADR 0031
adds an opt-in native-rendered public scene, Enter activation of the existing
bootstrap shell, and `desktop` return. Each QEMU session uses a private disk
copy so development can proceed alongside it. This removable test loop does
not complete the native service boundary.

**Bounded native workspace implemented and validated:** Proposed ADR 0032 introduces a
separate persistent ring-3 policy owner, portable canonical workspace core, and
software compositor. The implemented portable model supports four public demo
windows, two workspaces, local granted human/machine commands, focus and window
lifecycle, floating and tiled layouts, and declarative visual preferences.
The new `workspace-image`, `workspace-run`, `workspace-smoke`, and
`workspace-virtualbox` paths target this service. Native smoke passed 21 exact
complete frames through real keyboard input, with controlled-fault, watchdog,
malformed-mailbox, malformed-context, and orderly-exit containment; every
withdrawal restored resources and left a working bootstrap shell. The full
20-target validation matrix passed, including fresh desktop/workspace smoke and
final normal-image exclusion, with evidence tracked separately in the
[development log](../notebook/Development/2026-09-26%20Native%20Workspace%20Development%20Log.md).
This increment admits no arbitrary application process, private surface,
external agent connection, or executable extension.

**Native appearance and pointer increment implemented:** Proposed ADR 0033
introduces official Light/Dark themes, validated independent components, shared
paint/hit geometry, a task panel, pointer focus/move/resize, maximize/restore,
and a fresh shell-to-workspace return. Login/audio, external assets, durable
profiles, animations, and GPU effects remain unimplemented. Validation evidence
is tracked in the [theme and pointer log](../notebook/Development/2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md).

**Native frame asset increment implemented:** Proposed ADR 0034 introduces
original compiled vector controls, Clay-inspired Dark and interpreted Light
presets, validated frame-state components, rounded frames, blended shadows, and
canonical pointer-button state. The SVG reference package is generated from the
compiled source and does not add a runtime loader or exact framebuffer goldens.
Native smoke and the full 20-target regression matrix passed for this increment.
The final host compositor suite passed 31 tests after the last directed review
fixes. See the [Clay Frame Assets Log](../notebook/Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md).

**Built-in Appearance app implemented and validated:** Proposed
[ADR 0035](adr/0035-add-a-built-in-appearance-app-with-shared-choice-state.md)
adds a singleton settings window with explicit theme/component choices,
canonical keyboard selection, shared pointer geometry, and session-local
preferences. It preserves the current window capacity and admits no new app
process or private content. Bounded policy rejections keep the native owner
alive while unexpected failures still withdraw. All 20 validation targets
passed; native smoke verified 21 keyboard frames and 337 pointer/Appearance
frames across two sessions. The final model, theme, adapter, and compositor
suites passed. See the [Appearance App Log](../notebook/Development/2026-09-26%20Appearance%20App%20Log.md).
Future console work must introduce move-only private text storage and
disclosure-aware views before rendering sensitive input; the current public
`Copy` snapshot is not suitable for that content.

**Desktop interaction checkpoint:** Proposed
[ADR 0036](adr/0036-activate-desktop-controls-on-pointer-release.md)
adds release-to-activate buttons, canonical armed targets, and cancellation on
keyboard input, button chords, source loss, and scene changes. It retains
immediate window dragging and the existing presentation boundary. All 20
validation targets passed, including 21 native keyboard frames and 410
pointer/Appearance frames, of which 73 exercise release gestures. See the
[Desktop Click Lifecycle Log](../notebook/Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md).

**Mouse responsiveness follow-up validated:** the preceding request fixed
slow pointer interaction before selecting another development slice.
Measured work reduces composition and exact frame-transfer cost, and graphical
Linux QEMU now prefers KVM with a TCG fallback. Proposed
[ADR 0037](adr/0037-batch-ready-desktop-pointer-motion.md) additionally batches
at most 16 already-ready continuous motion packets without waiting. Raw
sequence checks, sequential edge clamping, original capture anchors, and
separate button/key/loss/source boundaries remain required. One semantic
update still requires a complete acknowledged frame. Final KVM isolated-input
medians are 8.9/11.2 ms for desktop/Appearance, and five sustained-input captures
match complete expected frames. All 20 validation targets and an additional
complete KVM workspace smoke passed. See the
[Mouse Responsiveness Log](../notebook/Development/2026-09-26%20Mouse%20Responsiveness%20Log.md).
**Current native graphics slice:** Proposed
[ADR 0038](adr/0038-add-native-window-content-surfaces.md) adds bounded immutable
RGB8 window content, caller-owned raster drawing, a distinct presentation grant,
and a move-only owner tied to the client generation and exact window
incarnation. Graphics Lab exercises atomic initial content and replacement
inside the existing trusted workspace vessel. Complete binding validation,
client-viewport clipping, and acknowledged full-frame publication preserve
window and semantic boundaries. All 20 validation targets and an additional
complete KVM workspace smoke passed, each verifying 23 exact graphics frames
plus retained interaction and containment proofs. Native timing and an exact
sustained-input capture also passed. Evidence is in the
[Window Content Surfaces Log](../notebook/Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md).

This is the rendering foundation for the user's Linux-first TRI-ABI direction;
foreign executable loading, syscall/runtime behavior, display protocols, and
independent app processes are not implemented by it. macOS's Darwin/XNU and the
Windows veil remain independent under ADR 0008. Filesystem and file-app work
remain future user-directed slices.

**Kernel follow-up selected:** the earlier desktop checkpoint proposed a
separate bounded dispatcher proof. That experiment is now implemented under
proposed [ADR 0039](adr/0039-prove-bounded-sustained-native-dispatch.md), with
final TCG/KVM evidence and all 23 validation targets passing as recorded above.
The workspace continues to run one unscheduled task and
withdraws its owner on failure. The new proof is not a persistent dispatcher
or an accepted production runtime architecture. General admission, wait/wake,
service loading, and restart policy remain broader foundation work below.

- Generalize the fixed proofs into arbitrary user address spaces, tasks, and
  bounded IPC with an explicit production scheduling and preemption policy.
- Define production sigil issuance, transfer, attenuation, and revocation beyond
  the fixed fixture tables.
- Generalize the fixed supervision and rite composition into production vessel
  launch, crash, restart, upgrade, terminal-exit, and service-lifecycle policy.
- Generalize ADR 0022's fixed reply, readiness, wake, and route-retirement proof
  and ADR 0023's accepted bounded host router into a production session path
  before moving a real shell session out of ring 0.
- Preserve ADR 0032's now-validated persistent private state, complete-frame
  presentation, and contained resource-restoring withdrawal as the native
  window service grows. Its first failure policy discards window state;
  it does not imply restart recovery.
- Admit independently supervised application clients through task-bound,
  revocable surface and input grants. Define private-content disclosure,
  focus/capture epochs, and app lifecycle before accepting arbitrary buffers.
- Add a versioned semantic IPC API so authenticated external machine clients
  and human interfaces reach the same canonical commands, observations, and
  policy. Local origin tags and public window IDs do not grant that authority.
- Extend the native pointer path with text input, declarative keymaps and persistent profiles, then
  measure damage tracking, multiple outputs, DPI behavior, and GPU composition.
  Preserve accessible state, keyboard paths, and nonvisual observation.
- Define recovery after loss of the canonical owner. ADR 0030's host model
  preserves one surviving coordinator across a fresh-presenter redraw; neither
  that model nor ADR 0032's withdrawal policy preserves windows after owner loss.
- Move the first device familiar out of ring 0.
- Make service and authority graphs inspectable.

Gate: a hostile vessel cannot read kernel or peer memory, and its failure does
not prevent supervised recovery.

## Milestone 3 — Shared semantic control plane

- Stabilize a canonical encoding and version negotiation for objects, rites,
  invocations, plans, progress, results, and audit events.
- Implement discovery, preview, authorization, cancellation, idempotency, and
  compensation semantics.
- Deliver a command client, an accessible shell projection, and a machine
  client over the same API and policy path. HAG compute services obey the same
  authority model; HAG is not the semantic API for agents.
- Establish the oracle trace and crash-artifact pipeline.

Gate: the same state-changing rite yields the same plan digest, sigil decision,
trace, and result regardless of which client initiates it.

## Milestone 4 — Reliquary and Covenant

- Specify and test the native Reliquary executable/package format.
- Build a constrained parser and loader vessel with fuzzed boundaries.
- Add signatures, provenance, SBOMs, governed context, and resource intent.
- Build the content-addressed store, `covenant_manifest` resolver, atomic
  generation activation, garbage collection, and rollback.
- Import one foreign package ecosystem through an isolated adapter.

Gate: power loss or validation failure cannot expose a partial generation, and
the exact plan approved by a person or agent is the one activated.

## Milestone 5 — Human system surface

- Add input, display familiars, a compositor, semantic object tree, and shell.
- Establish neumorphic tokens and components with contrast, focus, keyboard,
  assistive-technology, reduced-motion, and nonvisual-state conformance tests.
- Render Covenant, oracle, device, recovery, and permission experiences from
  the same schemas used by machine clients.

Gate: core workflows are operable with keyboard and screen reader, and no
important state depends only on color, depth, animation, or pixels.

## Milestone 6 — Compatibility veils

- Prove a Linux application tier first and publish its syscall/API coverage.
- Explore Windows API translation and isolated virtualization independently;
  terminate IA-32 execution plus 32-bit Win32 ABI pointer widths, handles,
  and calling conventions in a user-space compatibility veil that translates
  into x86-64 native rites.
- Complete technical and licensing feasibility work before selecting a macOS
  execution tier.
- Integrate foreign windows, files, notifications, and lifecycle controls via
  semantic bridges without granting ambient host authority.

Gate: every demonstrated application publishes its compatibility and isolation
tier, known deviations, provenance, and recovery behavior.

## Milestone 7 — AI and accelerator substrate

- Add IOMMU-confined accelerator familiars, memory placement, bounded zero-copy
  buffers, asynchronous queues, scheduling, accounting, and cancellation.
- Treat models, kernels, context, and provenance as governed artifacts.
- Qualify HAG's native Anomalous transport and measure its compute paths against
  direct workloads. Its intended Vulkan replacement remains gated on device,
  lifetime, correctness, and workload evidence; it is not required for the first
  software-rendered Antimatter surface.

Gate: representative AI workloads reach measured hardware-efficiency targets
without bypassing sigils, isolation, attribution, or operator controls.

## Milestone 8 — Breadth and stewardship

- Expand architectures, hardware, filesystems, networking, SDKs, and recovery.
- Harden the boot and update chain, isolation model, parsers, and supply chain.
- Publish compatibility matrices, performance baselines, support lifecycles,
  and stable interface commitments.
- Offer enterprise policy, certification, fleet, and support services atop the
  open protocols and MIT-licensed core.

Gate: release claims are backed by repeatable tests, threat-model coverage, and
clearly scoped maintenance commitments.
