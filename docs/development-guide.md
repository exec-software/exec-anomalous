# Development guide

This guide preserves the implementation checkpoints recorded through October 2026.
Test results are historical observations, not a fresh qualification of this public
source export. Start with the [project README](../README.md) for scope and entry points.
Legacy SDK commands require the [optional runtime requirements](optional-runtime.md);
the historical development SDK mentioned below is not included in this source release.

**A general-computing operating system for people and machines.**

Anomalous is an experimental operating system currently built from first
principles in Rust and narrowly scoped assembly. It aims to give human interfaces
and AI systems one coherent, inspectable, permissioned control plane: beautiful to use,
legible to automate, and possible to debug from the first instruction.

Anomalous is not a Linux distribution. It learns from decades of operating
system engineering while exploring a different substrate for execution,
context, observability, package management, and interaction. The north star is
deliberately ambitious: a seamless “last operating system” for a world in which
AI is a normal participant in computing.

> **Project status: post-milestone-0 bring-up, pre-alpha research.** The
> repository boots its own x86-64 loader and Rust kernel into a bounded
> bootstrap command environment. It now has bounded proofs for memory,
> interrupts, scheduling, supervision, a first native rite gate, one fixed
> composed shell-vessel lifecycle, and a host-only private session-ingress and
> waiting contract. A removable supervised ring-3 renderer now consumes a
> host-derived semantic-scene capsule, discards one faulted half-frame, and
> publishes an exactly verified replacement frame without mapping the
> framebuffer. A separate host-only desktop service now proves strict
> one-successor backpressure, one renderer replacement, and atomic semantic
> publication after a trusted presentation receipt. Native copied scene
> transport and one complete render/presentation composition are also proved.
> An opt-in development image adds an interactive desktop launcher and a
> returnable bootstrap shell. A separate native workspace increment now adds a
> bounded window model and software compositor in a persistent ring-3 vessel,
> with native mouse windowing, a task panel, and official Light/Dark themes.
> Its native smoke verifies repeated interactions and contained shell fallback,
> and the preceding theme/pointer increment passed all 20 validation targets.
> Clay-inspired frame assets and canonical control states now extend that work;
> all 20 validation targets passed for the frame increment as well. A built-in
> Appearance app now adds explicit settings and shared keyboard selection;
> all 20 validation targets passed for that increment. Release-to-activate
> controls now complete the desktop interaction checkpoint, with all 20 targets
> passing again. Mouse responsiveness has improved through measured
> compositor and transfer optimizations, bounded ready-motion batching, and
> hardware-accelerated interactive QEMU. All 20 validation targets and an
> additional complete KVM workspace smoke passed for this fix. The window-content
> increment adds bounded caller-rendered window content and a Graphics Lab.
> All 20 validation targets and a separate KVM workspace smoke passed,
> including 23 new exact graphics frames and retained interaction proofs.
> Native timing and an exact sustained-input capture also passed. This remains
> pre-alpha:
> general application processes, external agent IPC, a production scheduler,
> filesystem,
> package manager, compatibility layer, and production security remain open.
> Do not use it for production or sensitive workloads.

The preceding kernel foundation increment adds a removable sustained-dispatch
proof: 64 timer-driven switches between two private tasks, one contained exact
fault, and 16 further timer ticks with continued peer progress and complete
reclamation. All 23 validation targets passed, including the full regression
matrix and separate KVM dispatcher/workspace repeats. A separate VirtualBox
attempt was blocked by host KVM ownership of AMD-V; its isolated VM was cleaned
up without changing existing VMs or host settings. The
[dispatch log](../notebook/Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md)
records final evidence and limits.
The memory-planning checkpoint adds allocation-free planning for multiple user
regions and checked physical/kernel address translation geometry. Nineteen new
tests cover permissions, complete ranges, exclusions, shared page-table branches,
and independent resource budgets. Focused tests, Clippy, and all 23 regression
targets passed. The normal, dispatcher, and workspace images are byte-identical
to the preceding checkpoint. These plans prepare later owned page-table
construction; the live mapper retains its existing address restrictions. The
[mapping log](../notebook/Development/2026-09-26%20User%20Mapping%20Plan%20Log.md)
records this implementation and its remaining activation prerequisites.
The preceding increment builds real allocator-owned, multi-branch user page tables
through a temporary supervisor-only high-half physical window. Failed builds and
partial cleanup retain their leases for retry. Eleven new kernel tests and three
new transcript test groups passed. All 24 regression targets passed, including
the new native proof at 96 MiB and 320 MiB under both TCG and KVM. These user roots remain
inactive while the bootstrap kernel continues to use its original root. The
[owned construction log](../notebook/Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md)
records the proof and remaining kernel-residency/activation work.
The preceding resident increment adds explicit supervisor kernel and stack residency to two
owned roots and exercises real low-address mappings across four CR3 switches.
Exact CPL0 read-only, guard, and NX fault probes check hardware enforcement;
a retained high physical alias stays valid even when low virtual and physical
addresses collide. Cleanup restores the bootstrap root and all resources.
The [resident activation log](../notebook/Development/2026-09-26%20Resident%20Mapping%20Activation%20Log.md)
records validation and limits. All 24 regression targets passed, including both
RAM sizes under TCG and KVM. At that checkpoint, normal, dispatcher, and workspace
images matched the preceding checkpoint. This CPL0 bridge did not launch user
tasks or establish a permanent kernel layout.
The preceding increment runs two real GPR-only ring-3 tasks using owned low-address
mappings. Four exact private reports resume at noninitial user stack pointers;
a denied supervisor read terminates B, then A continues and exits. Entry checks
admit only declared initialized code and guarded stack bounds. Teardown restores
all 28 owned frames, eight leases, task/circle state, and transition state.
All 24 validation targets passed. TCG and KVM memory proofs passed at 96 MiB
and 320 MiB, restoring exactly 24,150 and 81,494 free frames. All 240 frozen
source/configuration files remained unchanged during validation.
The [user execution log](../notebook/Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records this Proposed ADR 0044 increment. Shared exception diagnostics now read
the actual hardware-saved RSP/SS for all long-mode frames; normal, dispatcher,
and workspace image hashes changed with that correction and their regression
proofs passed. This remains a removable fixture, not a general launcher or
syscall ABI.

The preceding increment, proposed
[ADR 0046](../docs/adr/0046-prove-retained-prepared-launch-ownership.md), adds a
retained `prepared_launch` owner for the same three-region GPR-only profile.
Preparation builds and verifies the payload/mappings and attaches circle
resources without publishing a task; explicit commit creates the ready task.
`cancel_unstarted` cancels a ready task before cleanup, and the retained owner
supports retries after partial mapping release. Thirteen native failure and
cancellation cases restore their resources before the two original users run
through this owner, including B's contained fault and A's continued exit.
Four task-transition tests, eight owner tests, seven memory-verifier tests,
freestanding Clippy, and 96-MiB/320-MiB TCG/KVM memory proofs passed. All 24
regression targets passed on 254 unchanged non-documentation files. Normal and
workspace images remain byte-identical to the recovery checkpoint. The
[Prepared Launch Ownership Log](../notebook/Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records the evidence at that checkpoint.

The preceding increment, proposed
[ADR 0047](../docs/adr/0047-prove-sustained-dispatch-over-retained-launches.md),
composes two retained launches with sustained dispatch over owned low-address
roots. It retains 36 frames and ten leases, including two 16 KiB supervisor
stacks reached through the high alias. Partial scheduler admission rolls back
without changing survivor order or sequence cursors; two failed stack clears
retain ownership for cleanup retries. The users complete 64 real preemptions,
one exact task-local UD2 fault, and 16 survivor ticks at noninitial nested stack
pointers. Both owners remain retained until both tasks are terminal.
Six scheduler-withdrawal tests, four task-withdrawal tests, six verifier test
groups, freestanding Clippy, and 96-MiB/320-MiB TCG/KVM proofs passed.
All 26 regression targets qualified across documented validation phases;
258 final frozen files remained unchanged after a narrow display-capture harness
correction. Normal/workspace images match recovery. Peak scope memory was
4.002 GiB under a 6 GiB cap, with no OOM event. The
[Owned Sustained Dispatch Log](../notebook/Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
records evidence and limits for that GPR-only checkpoint.

The preceding increment, proposed
[ADR 0048](../docs/adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md),
adds an internal x87/SSE2 context owner, checked copies of at most 4096 bytes
through the retained current-task owner, and selected task-local
#DE/#UD/#GP/#PF containment, including cleanup after the last current task
faults. The separate `application_profile_test` fixture completed four task
pairs, 256 preemptions, 320 timer ticks, and 20 successful/40 rejected copies
per VM. All 28 validation targets passed on 265 unchanged frozen source files,
including 96-MiB/320-MiB TCG and AMD-host KVM proofs. Normal and workspace images
remain byte-identical to the preceding checkpoint. The
[Application Profile Log](../notebook/Development/2026-10-03%20Application%20Profile%20Log.md)
records its status. TLS, AVX/XSAVE, a general loader, and a complete untrusted
application profile remain outside this increment.

The preceding proposed
[ADR 0049](../docs/adr/0049-qualify-a-bounded-freestanding-ny-object.md)
qualifies one real Ny-generated scalar object with an inspected freestanding
startup shim. The fixture checks copied initialized data, zero-filled globals,
integer/f64 computation, timer returns inside generated code, and two failure
paths followed by a successful peer. All 30 validation targets passed on 272
unchanged frozen files, including 96-MiB/320-MiB TCG and actual-host KVM proofs.
Normal/workspace images remain unchanged; no OOM event occurred. The
[Ny Object Log](../notebook/Development/2026-10-03%20Ny%20Object%20Log.md)
records final evidence and limits.
It uses the pinned compiler's unchanged object and a narrow runtime shim;
the hosted Ny runtime, a general target/loader, and Linux Antimatter execution remain open.

The preceding implementation, proposed
[ADR 0050](../docs/adr/0050-prove-a-bounded-native-application-contract.md), separates
a bounded Rust editor client and a surface/input service into independent tasks.
It ports real Antimatter selection replacement, theme preservation and undo into
a 31-byte ASCII model and 128×8 CPU renderer. Seven copied surfaces and six
scripted commands exercise task-bound transport, exit/fault revocation and a
deferred final display. All 33 matrix jobs passed on 291 unchanged frozen files,
including two-size TCG/KVM and capture proofs. The
[Native Application Log](../notebook/Development/2026-10-03%20Native%20Application%20Log.md)
records final evidence, the actual CodeMirror reference and zero OOM events.
Normal boot remains byte-identical; the shared presenter changed the workspace
image, whose TCG/KVM smoke tests passed. The host ELF build
container at that checkpoint was only a build input; this client is not the full editor.

The completed ELF checkpoint, proposed
[ADR 0051](../docs/adr/0051-prove-userspace-elf-loading-against-native.md), adds a
separate ring-3 loader that parses a bounded sectionless ELF64 deployment image.
It submits a copied inert plan; the kernel checks that plan against the trusted
manifest and retains allocation, grants and lifetime control. Loader resources
are reclaimed before the same editor/service client runs through native flat
spans and ELF-derived spans. The proof compares initialized data, zero-fill,
service results, surface hashes, pixels and exit/fault cleanup under the existing
native `int 0xf4` personality. Qualification completed on 2026-10-04: all 35
matrix jobs passed on 302 unchanged frozen source files, including both RAM
sizes and captures under TCG and actual-host KVM. The
[ELF Application Log](../notebook/Development/2026-10-03%20ELF%20Application%20Log.md)
records the evidence. ADR 0051 remains Proposed. This is a constrained
userspace ELF path, not Linux or Electron compatibility.

The preceding qualified increment, proposed
[ADR 0052](../docs/adr/0052-prove-linux-startup-and-record-device-syscalls.md),
adds a real Linux x86-64 startup/SYSCALL producer paired with the native service.
Its fixed read/write/poll/yield/exit subset exposes two nonblocking Anomalous
record devices for the same editor trace. Atomic input consumption, poll-array
copyback, queued-fault cleanup and exact CPU/entry-MSR restoration passed
qualification on 2026-10-04. All 37 matrix jobs and two image-restoration jobs
passed on 313 unchanged frozen source files, including 96/320-MiB TCG and
actual-host KVM boots and matching captures. The
[Linux Application Log](../notebook/Development/2026-10-04%20Linux%20Application%20Log.md)
records the bounded proof. This feature-private controller does not establish a
production userspace veil, Linux pipes, general Linux compatibility or Electron
support.

The preceding increment, proposed [ADR 0053](../docs/adr/0053-prove-a-supervised-userspace-syscall-veil.md),
implements a third independent ring-3 task for Linux syscall/device policy.
The existing Linux producer and native service retain their inspected bytes.
A sealed caller/veil relation blocks the caller, preserves the queued service,
and permits bounded checked copies and one inert staged effect. Completion
validates the exact request and scheduler continuation before publication;
a veil fault discards the stage and contains its dependent caller.
Qualification passed 39 matrix jobs and two restorations. The
[Userspace Veil Log](../notebook/Development/2026-10-04%20Userspace%20Veil%20Log.md)
tracks the native comparison, producer fault and staged-veil-fault scenarios.
This remains one fixed caller relation; startup construction stays in the
proof controller. ADR 0054 below extends its fixed caller relationship.

The preceding proposed [ADR 0054](../docs/adr/0054-prove-a-shared-multi-caller-userspace-veil.md) qualifies two independent Linux editor
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

The preceding proposed [ADR 0055](../docs/adr/0055-prove-native-startup-and-private-userspace-arenas.md) implements
copied native startup data and a private, guarded 32 KiB arena per task.
A bounded Rust editor allocates its state and transport/render buffers there,
using supplied document bytes and task-bound opaque handles. The proof covers
two documents/layouts, guard-write and NX faults, and fresh arena zeroing after
cleanup. **All 43 qualification checks and two image restorations passed**; the
[Native Runtime Log](../notebook/Development/2026-10-04%20Native%20Runtime%20Log.md) tracks evidence.
Proposed [ADR 0056](../docs/adr/0056-prove-task-bound-native-document-open-and-save.md) now implements a bounded native
RAM document service in a separate userspace task. The editor opens copied
content, saves an exact snapshot while making newer edits, and closes/reopens
committed bytes while that service survives. Lost replies retain an unknown,
unsynchronized state. Bounded native qualification passed; the
[Native Document Log](../notebook/Development/2026-10-04%20Native%20Document%20Log.md)
records the evidence. Proposed [ADR 0057](../docs/adr/0057-prove-elf-loading-of-the-native-document-contract.md)
qualifies the same bounded contract through the userspace ELF loader, retaining
the native `int 0xf4` personality and unchanged native client bytes.
The [ELF Document Log](../notebook/Development/2026-10-04%20ELF%20Document%20Log.md)
records the completed comparison and verified zero-tail compaction.

Proposed [ADR 0058](../docs/adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md)
adds native editor replacement while the same RAM document service stays alive.
Three cases distinguish unsaved edits, a discarded queued save and a committed
save whose reply is lost. Fresh task/channel generations and document handles
separate the replacement editor from the retired owner. The strict oracle checks
122 records, including independently specified document packets and surfaces.
All 50 qualification checks and two development-image restorations passed,
including TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with matching captures.
The [Native Relaunch Log](../notebook/Development/2026-10-04%20Native%20Relaunch%20Log.md)
records the completed evidence.

Proposed [ADR 0059](../docs/adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md)
implements the same three replacement scenarios through the userspace ELF loader
against the native baseline. Its strict 222-record oracle checks both routes
independently, including 48 surfaces, 138 document packets and retained-service
maintenance. The native personality and mapping/loader bounds remain unchanged.
All 52 qualification checks and two development-image restorations passed,
including TCG/Intel and KVM/AMD at 96 MiB and 320 MiB with matching captures. The
[ELF Relaunch Log](../notebook/Development/2026-10-04%20ELF%20Relaunch%20Log.md)
records the completed bounded comparison. Those RAM baselines retain saved bytes
only while their service lives. ADR 0063 now adds a separate native disk proof
with complete service destruction and cold recovery; a full Antimatter/Electron
port remains open.

The [execution-readiness audit](../docs/architecture/execution-readiness.md)
records remaining kernel gates and the modular order toward a native launch
contract, Ny userspace and Antimatter, Reliquary execution framing, Linux ELF,
and later Mach-O/Darwin.
The existing native Rite ABI is experimental; the native artifact format is
not yet implemented.

## Development direction

**Antimatter is the official editor. Ny is the official language direction for
Anomalous and HAG. HAG is the compute runtime intended to replace Vulkan**;
model and agent semantics remain in services above that compute layer.
[ADR 0045](../docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md)
accepts this product direction without claiming native integration readiness.
Existing verified Rust remains authoritative until qualified replacements
meet the relevant ownership, ABI, failure, and validation gates.

The next major userspace milestone is the existing Linux Antimatter application
launching, rendering and receiving input inside Anomalous through the Linux
compatibility backend. [ADR 0073](../docs/adr/0073-run-linux-antimatter-through-the-linux-veil.md)
selects its unchanged Electron/React/CodeMirror frontend; no native rewrite or
`.re` conversion is required. Useful editing then requires real file open,
edit, save and reopen. Software rendering may precede HAG acceleration.

[ADR 0074](../docs/adr/0074-plan-linux-images-and-initial-stacks.md) adds host-only
ELF64 multi-segment/PIE planning and Linux initial-stack construction, checked
against actual `true`, interpreter, libc and Electron artifacts. This prepares
owned mapping and startup work; it does not execute those binaries in Anomalous.
[ADR 0075](../docs/adr/0075-prove-owned-multi-region-launch.md) adds a default-off
native nine-region launch proof with separate initialized executable intervals,
high user stacks, retained initialization and complete cleanup. Its synthetic
payloads exercise the main/interpreter layout.
[ADR 0076](../docs/adr/0076-prove-owned-loader-sources.md) now implements copied,
sealed native loader sources that survive producer retirement, qualified under
QEMU TCG/KVM and VirtualBox. The [Owned Loader Sources Log](../notebook/Development/2026-10-07%20Owned%20Loader%20Sources%20Log.md)
records the source lifetime, cleanup and regression evidence. Actual Linux image integration,
TLS and Linux mapping/file services still gate unchanged execution.
The native reference editor and the following bounded proofs remain regression
baselines. ADR 0049 qualified one bounded Ny object/startup composition against
the internal application profile.
The selected ELF loading comparison is now qualified as a bounded proof under
proposed ADR 0051. The Linux Antimatter copy outside the VM remains the
behavioral reference; ADR 0052 qualifies the separate bounded guest Linux
startup/syscall route. ADR 0053 implements the bounded userspace policy
relocation. ADR 0054 qualifies a shared two-caller service. ADR 0055 implements
native startup and private arenas, with bounded qualification complete. ADR 0056
qualifies bounded RAM document open/save within that fixed proof.
ADR 0057 qualifies the same bounded contract's ELF comparison. ADR 0058 adds
native editor replacement with the service retained; bounded qualification is complete.
ADR 0059 qualifies the same bounded retained-service replacement comparison
through the userspace ELF loader. ADR 0063 now qualifies normal-path native disk
save and cold recovery, including bounded native fault cuts and lost-reply
reconciliation. General filesystem services remain open. Windows and macOS
comparisons remain later work.
Generated code must fit the admitted CPU and runtime profile. Linux Antimatter
execution, a freestanding Ny application runtime, and HAG integration remain unimplemented. The
[Native Antimatter Direction Log](../notebook/Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
records the source audit and milestone boundaries.

## Filesystem direction

There is no general filesystem yet. The earlier editor proofs save in RAM;
ADR 0063 now connects ADR 0060's recovery model to a separately granted data disk
for a bounded native editor/service proof.
Accepted [ADR 0062](../docs/adr/0062-design-native-object-filesystem.md) now selects
the [native filesystem architecture](../docs/architecture/filesystem.md): stable
objects independent of names, versioned operations, streamed names and paths
without fixed global component/length/depth ceilings, and copy-on-write project
transactions. Snapshots, independently qualified replicas, and verifiable full
or incremental backups each have their own recovery contract.

Proposed [ADR 0063](../docs/adr/0063-prove-native-durable-document-storage.md) now
implements exact native save to a dedicated data disk, complete editor/service
destruction, remount and recovery in a fresh VM process. QEMU tests at 96 MiB
and 320 MiB independently verify disk bytes, rendered text, revoked authority,
and unchanged boot/outside-extent bytes. Recovery receives no expected text.
The [Native Durable Document Log](../notebook/Development/2026-10-04%20Native%20Durable%20Document%20Log.md)
records the evidence and remaining fault/lost-reply cases. The five-sector,
31-byte format is a proof fixture, not a permanent document or naming limit.

## What boots today

The repository produces a bootable 16 MiB disk image:

- **Threshold**, the Anomalous boot loader, owns the path from a 512-byte BIOS
  stage through its versioned disk manifest, independently loaded second stage,
  VBE and E820 discovery, verified A20, protected mode, page tables, and x86-64
  long mode. It does not use GRUB.
- The Rust kernel is a separate, hashed image loaded in bounded BIOS chunks,
  staged below conventional-memory limits, and copied to its linked 1 MiB
  address before long-mode entry.
- A version-2 **boot_omen** record forms the contract between Threshold and the
  Rust kernel. Threshold publishes its bounded E820 table only after complete
  enumeration; the kernel copies and independently validates every descriptor.
- The kernel replaces the loader's descriptor state with a kernel-owned GDT and
  TSS, installs all 32 x86-64 CPU exception gates, and reports fatal faults with
  attributable machine state. Double fault and NMI have dedicated stacks; NMI
  remains masked pending a separate injection test.
- The heap-free Rust kernel normalizes whole frames below four GiB into a fixed
  two-bit ledger, reserves low memory, the linked kernel, and the framebuffer,
  and grants only opaque, generation-checked leases. It emits bounded memory
  state over COM1 and renders the matching memory-ready state for people.
- QEMU provides deterministic, 15-second-bounded smoke tests. Normal images
  contain no test-only exit request, and VirtualBox is the primary interactive
  bring-up environment.
- A separate `circle_test` image proves one fixed ring-3 boundary under QEMU
  and VirtualBox: exact supervisor-read and NX-stack denials, destructive
  teardown, and a clean third generation. The gate, payload, and recovery path
  are absent from normal images and are not a general process model.
- A separate `memory_test` image creates two allocator-backed roots that map one
  virtual address to distinct RW+NX frames, switches A-to-B-to-A, restores the
  kernel root, withdraws both mappings, and proves zeroed deterministic reuse.
  The strict QEMU proof runs at 96 MiB and 320 MiB, explicitly rejects virtual
  overlap, duplicate binding, and active-root destruction, and has been
  reproduced by the dedicated 512 MiB VirtualBox VM. ADR 0012 records the
  accepted evidence and bootstrap-only limitations.
- The BSP-only `legacy_pc` backend owns returning IRQ vectors, dual 8259 PICs,
  PIT channel 0, and the 8042 keyboard path. It exposes same-timebase monotonic
  ticks and fixed typed physical-input delivery with exact loss, bounded device
  reinitialization, and generational rearm. Fresh QEMU and VirtualBox proofs
  each observed initial masks 255/255, active masks 252/255, eight timer IRQs,
  generation-1 input sequences 0 through 23, exact loss at sequence 88,
  containment at master mask 254, generation-2 rearm and sequences 0 through 1,
  and final masks 255/255. NMI, APIC routing, scheduling, and general timers
  remain outside this slice.
- The experimental **invocation** header begins the typed control-plane ABI.
- Allocation-free `rite_call` and `rite_reply` prefixes, a task-bound
  architecture-neutral admission core, and one fixed feature-only x86-64 gate
  now prove a narrower kernel/user operation boundary. The gate remains absent
  from normal images and is not a stable syscall surface or production ABI.
- A separate `shell_vessel_test` image composes that rite boundary with real
  PIT scheduling and one-restart supervision. Fixed human-source and
  machine-direct projections commit identical console semantics across
  destructive replacement; terminal exit wakes and selects the supervisor
  without replying or returning to the exited vessel. This is a removable
  mechanism proof, not Canticle or a production shell/runtime.
- A separate `display_vessel_test` image gives one ring-3 task only
  `display_update | task_exit`. The kernel copies exact versioned RGB8 patches
  into private scratch and presents them through its exclusively owned
  framebuffer; ring 3 receives no framebuffer mapping or device geometry. The
  strict QEMU proof preserves the complete aperture across three rejected
  calls, commits 768 rows, tears down the task, and independently matches all
  786,432 captured pixels. This is a removable presentation proof, not a
  compositor, window server, or semantic desktop.
- A separate `supervised_renderer_test` image composes the canonical host-only
  scene with a fixed-address ring-3 renderer and one persistent supervisor.
  Generation 1 survives one real PIT preemption, stages 384 private rows, then
  faults at an exact `#UD`; its whole stage and task state are withdrawn before
  a fresh generation stages all 768 rows and exits terminally. Only that
  complete replacement frame is published. The retained strict-P6 capture
  matches an independent semantic oracle at all 786,432 pixels, while the
  renderer receives neither scene authority nor framebuffer access. This is a
  removable composition proof, not a live scene service, compositor, window
  server, or normal-image desktop.
- The host-only, allocation-free `anomalous-desktop-service` crate keeps one
  complete semantic revision public while at most one successor moves through
  queued, leased, ready, and presented phases. It returns exact
  backpressure ownership, permits one fresh renderer retry over the same
  immutable redacted snapshot, and publishes human, accessibility, and machine
  semantics together only after consuming the presentation receipt. It is not
  linked into the kernel or a renderer image and does not define native
  transport, verified pixels, a window server, or a stable graphics ABI.
- The host-only `anomalous-desktop-recovery` coordinator extends that surface
  service with explicit unavailability between framebuffer-copy authorization
  and final acknowledgement. It retains the exact redacted successor for one
  fresh presenter to redraw after an interrupted presentation, without
  publishing its semantic revision twice. This is a local recovery contract
  documented in proposed [ADR 0030](../docs/adr/0030-prove-bounded-presentation-recovery-coordination.md),
  not a native service or evidence that a framebuffer was actually copied.
- Proposed [ADR 0032](../docs/adr/0032-start-a-native-window-workspace-and-compositor-service.md)
  starts a distinct workspace service: four public demo windows across two
  workspaces, reversible focus and window lifecycle, floating and tiled layouts,
  and declarative visual settings. The allocation-free `no_std` workspace and
  compositor crates keep desktop policy above the kernel. Local human and
  machine endpoints use the same granted typed commands; this does not provide
  an external agent connection or arbitrary application surfaces. The native
  smoke passed 21 exact complete frames and five contained termination paths;
  the complete validation matrix passed. Measured evidence is recorded in the
  [workspace development log](../notebook/Development/2026-09-26%20Native%20Workspace%20Development%20Log.md).
- Proposed [ADR 0033](../docs/adr/0033-build-the-first-native-themes-and-pointer-window-shell.md)
  extends that service with official Light/Dark themes, validated independent
  appearance components, native PS/2 mouse window controls, and a task panel.
  The final smoke verified 124 pointer frames across two desktop sessions in
  addition to the 21 keyboard frames and all five containment paths. All 20
  validation targets passed; current evidence is in the
  [theme and pointer log](../notebook/Development/2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md).
- Proposed [ADR 0034](../docs/adr/0034-build-shared-native-frame-assets-and-canonical-control-states.md)
  adds original compiled vector window controls, Clay-inspired Dark and warm
  interpreted Light, validated frame-state components, rounded silhouettes,
  blended shadows, and canonical pressed state. The generated
  [frame asset references](../assets/desktop/frames/README.md) share native control
  geometry but are not exact framebuffer goldens or a runtime asset package.
  Native smoke and all 20 validation targets passed, with evidence in the
  [Clay Frame Assets Log](../notebook/Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md).
- Proposed [ADR 0035](../docs/adr/0035-add-a-built-in-appearance-app-with-shared-choice-state.md)
  adds the singleton Appearance app, sixteen explicit choices, canonical
  keyboard selection, and recoverable bounded policy rejection. Mouse,
  keyboard, and local machine clients use the same settings and granted actions.
  All 20 validation targets passed. Native smoke verified 21 keyboard frames
  and 337 pointer/Appearance frames across two sessions; evidence is in the
  [Appearance App Log](../notebook/Development/2026-09-26%20Appearance%20App%20Log.md).
- Proposed [ADR 0036](../docs/adr/0036-activate-desktop-controls-on-pointer-release.md)
  makes window, panel, and Appearance buttons activate on matching release.
  Canonical armed feedback follows the original control; moving away before
  release cancels the action. Keyboard input, button chords, source loss, and
  competing scene revisions also cancel. All 20 validation targets passed;
  native smoke verified 21 keyboard and 410 pointer/Appearance frames.
  The [Desktop Click Lifecycle Log](../notebook/Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md)
  records that desktop checkpoint. The current
  [Mouse Responsiveness Log](../notebook/Development/2026-09-26%20Mouse%20Responsiveness%20Log.md)
  records the measured improvement and final passing validation.
- Proposed [ADR 0038](../docs/adr/0038-add-native-window-content-surfaces.md)
  introduces validated immutable RGB8 window content, a window-specific
  presentation owner, and complete-frame publication of content metadata.
  Graphics Lab exercises caller-owned drawing inside the existing trusted
  workspace vessel. All 20 validation targets and an additional complete KVM
  workspace smoke passed, each verifying 23 exact graphics frames plus the
  retained interaction and containment proofs.
  The [Window Content Surfaces Log](../notebook/Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md)
  tracks this increment. This creates a native graphics foundation without
  implementing a Linux, Windows, or macOS compatibility veil.
- The host-only, allocation-free `anomalous-session` crate proves bounded
  semantic actions, private move-only storage, exact FIFO backpressure,
  generational session replacement, and one lost-wake-safe consumer wait. It is
  not linked into the kernel or native rite transport and does not authenticate
  an origin, securely erase hardware memory, or provide a shell runtime.
- Normal images continue into an allocation-free framebuffer or VGA-text
  bootstrap shell that drains interrupt-fed PS/2 transitions through a separate
  provisional US ASCII adapter. It supports the read-only `help`, `version`,
  `about`, `clear`, and `echo` commands. Its fixed 128-byte editor and parser are
  host-tested. Fresh QEMU and VirtualBox checks corrected `versiom` with
  Backspace plus `n` and dispatched `version`; QEMU also verified privacy-safe
  `echo` and unknown-command paths.
- Bootstrap-shell telemetry records only static command classifications and
  outcomes. It does not copy raw commands or `echo` arguments into the serial
  event stream. Raw scan bytes and physical-key records remain kernel-private;
  COM1 is output-only and is not an AI or human command surface.

## The thesis

Human-friendly and machine-native are not competing modes. They are two
projections of the same typed system model.

Every meaningful system action should become discoverable, permissioned,
previewable, observable, and attributable whether it is initiated through a
graphical surface, a shell, or an AI agent. Model and agent services use this
shared control plane. HAG supplies the planned compute layer beneath them;
compute submission does not bypass kernel authority or user policy.

The architecture is being shaped around:

- a small capability-oriented kernel and restartable user-space **vessels**;
- explicit **sigils** of authority instead of ambient privilege;
- structured events, trace identity, crash artifacts, and inspectable state;
- the **Covenant** package system with atomic transactions and rollback;
- the metadata-rich **Reliquary** native artifact format;
- accessible neumorphic interfaces backed by a semantic object tree; and
- isolated compatibility **veils** for Linux, Windows, and macOS applications.

These are design goals unless the current-status section says otherwise.

## Build and boot

The supported bring-up host is currently x86-64 Linux. Install:

- rustup;
- GNU Make and Bash;
- Python 3.11 or newer for the isolated VirtualBox runner and its host tests;
- GNU binutils (`as`, `ld`, `nm`, `objcopy`, and `addr2line`);
- QEMU for the automated smoke test; and
- VirtualBox 7 for the primary interactive workflow.

The repository pins its nightly Rust snapshot and required components in
[rust-toolchain.toml](../rust-toolchain.toml).

~~~sh
make check       # format, unit tests, Clippy, and script syntax
make image       # build build/anomalous.img
make smoke       # boot headlessly and require the boot.ready event
make fault-smoke # trigger and verify an exactly attributed invalid opcode
make circle-smoke # prove the fixed three-generation ring-3 boundary
make control-smoke # prove task-local authority and attributable copied IPC
make rite-smoke # prove the accepted fixed feature-only x86-64 rite gate
make shell-vessel-smoke # compose rites, scheduling, restart, and terminal wake
make display-vessel-smoke # prove bounded ring-3 framebuffer presentation
make display-demo # leave the proved public frame visible in a QEMU window
make supervised-renderer-smoke # prove fault-discard and replacement publication
make supervised-renderer-demo # display the same inspected supervised frame
make semantic-surface-transport-smoke # prove one copied native scene delivery
make semantic-presentation-smoke # verify native rendering and host publication
make scheduler-smoke # prove bounded preemption and contained user failure
make dispatcher-smoke # prove sustained preemption, sole-peer progress, and reclamation
make owned-dispatcher-image # build the separate retained-launch dispatch proof
make owned-dispatcher-smoke # prove owned-root dispatch and rollback at 96/320 MiB
make application-profile-image # build the bounded x87/SSE2 application proof
make application-profile-smoke # prove CPU state, checked copies, and user faults at 96/320 MiB
make ny-object-inspect # qualify the pinned Ny object and startup; requires its SDK
make ny-object-image # build the separate Ny object image; requires its SDK
make ny-object-smoke # prove the Ny startup/failure/peer paths at 96/320 MiB
make native-application-inspect # inspect the bounded Rust client build artifact
make native-application-image # build the separate native surface/input proof
make native-application-smoke # exercise copied editor/service tasks and final display
make native-runtime-inspect # inspect the copied startup/arena client artifact
make native-runtime-image # build the separate guarded native arena proof
make native-runtime-smoke # qualify startup, arena copies, faults and reclamation
make native-document-inspect # inspect the native document client/service artifact
make native-document-image # build the bounded RAM document proof
make native-document-smoke # exercise open/save/reopen and uncertain save outcomes
make elf-document-inspect # inspect zero-tail compaction and the unchanged loader bounds
make elf-document-image # build the native/ELF document comparison
make elf-document-smoke # compare exact saves, uncertain faults and final pixels
make native-relaunch-inspect # inspect the native editor-replacement artifact
make native-relaunch-image # build replacement with a retained RAM document service
make native-relaunch-smoke # verify saved bytes, stale authority and service continuity
make native-durable-build # inspect and build the separate disk-backed native fixture
make native-durable-smoke # verify full pair destruction, cold recovery, media and pixels
make elf-relaunch-inspect # inspect ELF replacement against the native fixture
make elf-relaunch-image # build adjacent native/ELF retained-service comparisons
make elf-relaunch-smoke # verify both routes against exact replacement checkpoints
make elf-application-inspect # inspect client, compact ELF, loader and inert plan
make elf-application-image # build the separate userspace ELF loading comparison
make elf-application-smoke # compare native/ELF loading, pixels and cleanup
make linux-application-inspect # inspect the bounded Linux SYSCALL producer
make linux-application-image # build the Linux producer/native service comparison
make linux-application-smoke # compare the same editor trace through Linux syscalls
make veil-application-inspect # inspect the independent userspace veil and retained clients
make veil-application-image # build the three-task supervised veil comparison
make veil-application-smoke # prove policy relocation and dependent-caller containment
make shared-veil-inspect # inspect unchanged client/service/veil artifacts
make shared-veil-image # build the five-task shared veil proof
make shared-veil-smoke # prove two-caller isolation, survival and service-loss containment
make supervision-smoke # prove one bounded supervised-vessel restart
make memory-smoke # prove bounded ownership at two emulated RAM sizes
make interrupt-smoke # prove returning PIT/keyboard IRQs, loss, and privacy
make normal-exclusion # reject enumerated feature-test symbols and payload markers
make shell-smoke # inject keyboard commands and verify command-text redaction
make workspace-image # build the separate native window workspace image
make workspace-run # launch the persistent workspace development vessel
make workspace-smoke # verify interactions, composition, and contained fallback
make workspace-virtualbox # launch the workspace image in VirtualBox
make desktop-run # build and launch the earlier bootstrap desktop image
make desktop-smoke # verify native frame, shell input/privacy, and desktop return
make desktop-virtualbox # build and launch that desktop in VirtualBox
make run         # interactive QEMU boot
make virtualbox  # create/update and launch the development VM
~~~

The Ny object commands are opt-in and require the shared Anomalous runtime at
`runtime/` to resolve to the exact immutable generation pinned by ADR 0049. They
fail explicitly if that runtime or any pinned tool is missing or changed.
Default `make check` needs no Ny runtime: it includes the pure
artifact/transcript tests and kernel feature checks, while image construction
and native Ny proofs run through the explicit commands above. No runtime-dependent
image check is silently skipped.

The native application proof uses Rust and the experimental native contract.
`node scripts/native-application-reference.cjs /path/to/exec-antimatter` checks
its seven-state reference against that checkout's actual CodeMirror and editor
bridge using existing installed dependencies; it launches no Electron window.

`make workspace-run` is the current windowing development path. It builds the
separate `build/workspace-dev.img` and launches **Anomalous Workspace
Development**. Its persistent ring-3 vessel owns both canonical window state
and CPU composition. Welcome, System Inspector, Activity, Appearance, and
Graphics Lab are built-in public content inside that owner, not separate
processes. The initial view has two windows,
with room for four across two workspaces. The mouse can focus windows, drag
titlebars, resize from the lower-right corner, and use minimize/maximize/close
controls. The bottom panel restores windows, switches workspaces, opens another
demo, and changes appearance.

To try window content, press **F2** for **Graphics Lab**, then **F3** while it
is focused to switch between the geometry scene and chart. Drag or resize the
window, or press **X** to maximize/restore it; the content scales within its
frame. **F2** brings an existing Graphics Lab to the current workspace and
retains its selected scene. The final validation status for this increment is
in the [Window Content Surfaces Log](../notebook/Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md).

Graphical QEMU runs use hardware acceleration when available on Linux and fall
back to software emulation when KVM cannot initialize. Set
`ANOMALOUS_QEMU_ACCEL=auto`, `kvm`, or `tcg` to choose the policy; explicit
`kvm` requires that backend, while `auto` is the default. On other hosts, `auto`
uses TCG. Automated proof runners retain their existing software backend. The
[Mouse Responsiveness Log](../notebook/Development/2026-09-26%20Mouse%20Responsiveness%20Log.md)
records measured input-to-frame timing and current validation status.

Proposed [ADR 0037](../docs/adr/0037-batch-ready-desktop-pointer-motion.md) bounds
continuous motion to at most 16 already-ready packets per canonical frame. Each
raw sequence is still validated, and each movement is clamped in order. Button
transitions, loss, keys, and source changes remain separate; one boundary
record can be retained untouched for the next iteration. The adapter never
waits to collect motion, and drag/resize retain their original capture anchor.
Every resulting update still needs the full acknowledged frame before shared
publication. The final KVM image measured 8.9/11.2 ms median input-to-complete-
frame time for desktop/Appearance across 24 samples each. Five sustained-input
captures matched expected complete frames. All 20 validation targets and an
additional complete KVM workspace smoke passed.

Buttons activate when the left mouse button is released over the same control
that was pressed. Move away before releasing to cancel; moving away and back
keeps the original press. Keyboard input, extra mouse buttons, or input loss
cancel it. A fresh pointer source must first observe all buttons released.
Window focus, dragging, and resizing remain immediate. The pressed target is
canonical state shared with human, accessibility, and machine views. See
[ADR 0036](../docs/adr/0036-activate-desktop-controls-on-pointer-release.md) and the
[Desktop Click Lifecycle Log](../notebook/Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md)
for the checkpoint and validation status.

The **Applications** panel button or **F1** opens a launcher for Welcome, System
Inspector, Activity, Appearance, and Graphics Lab. Arrow keys or Tab select;
Enter or Space opens the selected app. Escape closes the menu before a second
Escape returns to the shell. Outside clicks dismiss without activating a window
underneath. Existing app windows are restored on the current workspace; absent
apps are disabled when the four window slots are full. These are built-ins,
not general executable loading. See the [launcher contract](../docs/architecture/desktop-launcher.md).

The panel now contains Applications, running windows, workspace switching and
Appearance. The **Appearance** panel button, launcher entry or **P** opens a settings window with explicit
theme, density, bitmap text size, cursor shape, frame-corner, shadow, and motion
choices, plus reset. Reopening restores the same window on the current
workspace. Its default 840×568 geometry shrinks to fit the work area; very small
or tiled windows show a maximize prompt instead of hidden active controls.
Selected settings, keyboard focus, hover/press, and disabled choices remain
visible. Incompatible retained overrides disable a choice before activation;
reset can also be disabled if default workarea geometry would strand windows.

The official **Anomalous Dark** theme takes its charcoal surfaces, warm text,
orange focus, and teal accents from Antimatter's Clay direction. **Anomalous
Light** interprets the same roles with warm light surfaces and darker chromatic
inks. High Contrast remains an accessibility preset, and Light remains the
initial theme. Colors, chrome metrics, decoration-state colors, and frame style
can be overridden independently in the typed model after complete validation.
Appearance contains density, embedded bitmap window-text size, and cursor
style. Panel controls retain a bounded readable size. The desktop itself will
receive a separate name later.

Original compiled vectors draw minimize, maximize, restore, and close controls.
Floating frames have rounded corners and optional blended shadows; tiled and
maximized frames stay square. Hover, pressed, inactive, and disabled control
states derive from the same canonical snapshot as window actions. The
[generated SVG reference package](../assets/desktop/frames/README.md) documents
the geometry and states, while native bitmap text and pixels remain the
compositor's responsibility.

| Key | Workspace action |
| --- | --- |
| **Tab** | Focus the next visible window |
| **P** | Open or restore Appearance on the current workspace |
| **F2** | Open or restore Graphics Lab on the current workspace |
| **F3** | Switch scenes while Graphics Lab is focused |
| **F6**, **Shift+F6** | Select the next/previous Appearance choice when its controls fit |
| **Enter**, **Space** | Apply the selected enabled Appearance choice |
| **T** | Cycle floating, horizontal tile, and vertical tile layouts |
| **C**, **D** | Cycle theme; toggle compact/comfortable density |
| **Arrow keys**, **Shift + arrow keys** | Adjust the focused window’s floating position/size |
| **N**, **Q** | Create a public demo window; close the focused window |
| **M**, **R** | Minimize the focused window; restore a minimized window |
| **1**, **2**, **W** | Switch workspace; move the focused window to the other workspace |
| **G**, **A** | Toggle static depth and reduced-motion preferences |
| **F**, **V**, **X** | Toggle window text size, cursor style, and maximize/restore |
| **Esc** | Withdraw the workspace and return to the bootstrap shell |
| **F11**, **F12** | Exercise watchdog termination and controlled-fault fallback |

High Contrast forces effective flatness while retaining the depth preference.
Motion preference is canonical state; current effects are static depth and
focus decoration, with no animation backend. Login screens, sounds, external font/theme packages, durable
settings, real application clients, and remote agent access remain future work.
**Esc**, **F11**, and **F12** end this workspace session. At the fallback shell,
type **`desktop`** to open a fresh native workspace; prior windows and appearance
are discarded. QEMU releases keyboard/mouse capture with **Ctrl+Alt+G**. Close
the QEMU window to stop the VM.

Graphics Lab uses two opaque RGB8 scenes drawn once at service startup through
the portable `anomalous-window-surface` canvas. Both frames remain immutable;
F3 selects the other frame through the content-update path. The compositor
stretches immutable content
inside the shared client viewport while preserving desktop chrome and window
occlusion. Each published window exposes a public content label, descriptor,
and content sequence. A distinct `present` grant and move-only window owner
control replacement; pixels and metadata become current through the existing
complete-frame publication path. The fixed demo has no independent app process,
private content, general accessibility tree, or foreign ABI. Component and native
measurement evidence and the complete passing regression matrix are in the
[Window Content Surfaces Log](../notebook/Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md).

The typed model exposes `appearance_choice::all`, next/previous traversal,
`render_view::appearance_selected` and `appearance_enabled`, and each window's
`appearance_selection`. `open_appearance` requires manage plus arrange grants;
`select_appearance` requires arrange; `activate_appearance` requires arrange
plus customize. Every command also requires observation and the current
revision. Enabled checks normalize a copied candidate without changing the
owner or reserving a transaction. Bounded geometry, capacity, theme, and layout
rejections retain the session; unexpected authority, input-source,
transaction, or presentation failures still withdraw it.

The shared `desktop_target` identifies controls; `render_view::pointer_armed`
exposes the current press without granting its action. `arm_pointer` and
`cancel_pointer` require observe and arrange authority. Every successor clears
the arm unless a valid continuation explicitly rearms it. Activation retains
the ordinary window/settings command and its full grants. An unmapped key or
rejected keyboard action still publishes cancellation when a control was armed.

`make workspace-image` builds without launching. A graphical launch uses a
private per-run disk and bounded owner-only logs under `build/desktop-runs/`,
then releases the shared build lock so the VM can stay open during development.
`make workspace-virtualbox` selects the separate workspace image in the existing
**Anomalous Dev [Threshold]** VM, which must first be powered off. See the
[Appearance App Log](../notebook/Development/2026-09-26%20Appearance%20App%20Log.md)
for current validation status. The [Clay Frame Assets Log](../notebook/Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md)
and [theme and pointer log](../notebook/Development/2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md)
record preceding full passes and host limitations; the earlier
[workspace development log](../notebook/Development/2026-09-26%20Native%20Workspace%20Development%20Log.md)
records the keyboard-only baseline.

`make desktop-run` opens the opt-in desktop in a QEMU window. Press **Enter**
on the focused **Open shell** control, use `help`, `version`, `about`, `clear`,
or `echo`, then type **`desktop`** to return to the launcher. QEMU releases
keyboard/mouse capture with **Ctrl+Alt+G**. Close the window to stop the VM.
In this earlier launcher, the secondary control is disabled and there is no
mouse interaction or window lifecycle. This is a native-rendered bootstrap
launcher with the existing kernel-hosted shell, not the persistent user-space
desktop service.

`make desktop-image` builds `build/desktop-dev.img` without starting it.
The separate image is preserved when normal regression tests rebuild
`build/anomalous.img`. Each QEMU launch uses its own private disk copy, so you
can rebuild and run tests while the desktop stays open. Its bounded private
logs live under `build/desktop-runs/run-*/`. `make desktop-virtualbox` uses the
stable desktop image in the existing **Anomalous Dev [Threshold]** VM, which
must first be powered off. Desktop
serial logs stay in private files; they contain static event metadata and
command kinds, not typed shell text. See proposed
[ADR 0031](../docs/adr/0031-add-an-interactive-bootstrap-desktop-development-image.md)
for the bounded runtime and the development log for measured evidence.

`make rite-smoke` builds the fixed x86-64 proof accepted by ADR 0019. Its
strict 20-record QEMU transcript covers nine ordered ring-3 calls, six
payload-free rejections, two commits, one empty-input `would_block`, terminal
exit without `iretq`, and exact teardown. The gate, fixture, private marker,
helpers, and event namespace remain absent from the normal image.

`make shell-vessel-smoke` builds the composed proof accepted by ADR 0020.
Its strict 38-record QEMU transcript covers five rites across two destructive
incarnations, one real PIT preemption, one contained `#UD`, fresh channel
sequence after restart, equal fixed human/machine console semantics, terminal
supervisor selection, and exact reclamation. Private source, output, traces, and
markers stay out of serial, and the complete feature stays out of the normal
image.

`make display-vessel-smoke` builds the presentation proof accepted by ADR
0024. Its strict 16-record transcript covers three failure-atomic rejections,
768 full-width RGB8 commits, terminal exit, and exact reclamation. The retained
1024x768 strict-P6 capture must match an independent oracle at every pixel.
`make display-demo` boots the same inspected feature image for end-user visual
review; it does not add desktop semantics or framebuffer authority to ring 3.

`make supervised-renderer-smoke` builds the composition proof accepted by ADR
0026. It compares two protected-source target ELFs byte for byte, inspects a
6,469-byte two-page executable load image, exercises one real preemption and
one exact fault across two renderer incarnations, and requires exact teardown before and
after one full-frame publication. The retained 1024x768 strict-P6 capture must
match the independent canonical-scene oracle at every pixel.
`make supervised-renderer-demo` boots that same inspected payload for visual
review; it does not create a general renderer protocol or grant scene/action
authority to pixels.

ADR 0021 accepts the host-only `anomalous-session` proof at implementation
checkpoint `a2c72b2`. Its 51 unit tests and 56 compile-fail documentation tests
passed with warning-denied Clippy. The complete established host, QEMU, and
normal-image exclusion matrix also remained green. VirtualBox was neither run
nor required because this slice changes no boot or kernel code.

The VirtualBox target creates a dedicated VM named
**Anomalous Dev [Threshold]** by default. Override it with
`ANOMALOUS_VM_NAME` if needed, or set `ANOMALOUS_VM_FRONTEND` to `headless` for
a non-graphical VM process. Serial output is written to
`build/virtualbox.log`. Set `ANOMALOUS_VM_START=0` to refresh the VM and disk
without starting it, which is useful when another hypervisor currently owns
hardware virtualization.

After `make control-smoke` leaves the feature image in `build/`, run
`ANOMALOUS_VM_MODE=control ./scripts/run-virtualbox.sh` for a bounded headless
run whose captured lifecycle is checked by the same strict transcript parser.
Run `make image` afterward to restore the normal interactive disk image.

For an isolated capture of an **already built** image, use the bounded proof
runner. Its output directory must be new; its private VirtualBox registration
does not adopt the interactive VM or replace its disk:

~~~sh
python3 scripts/virtualbox-proof.py --image build/anomalous.img \
  --output build/virtualbox-normal-run --expect-event boot.ready
~~~

Each run also gets a unique `VBOX_IPC_SOCKETID`: changing the settings directory
alone does not isolate an already-running VirtualBox background service.
The runner uses one CPU and 512 MiB, disables network/USB/audio, retains the raw
image, VDI, VM configuration, command receipts, serial log, PNG and process
cgroups, then powers the VM off. Each CLI call and guest wait has a deadline.
Run it inside the development memory limits and serialize it with QEMU/KVM.
Seeing an event is only a capture trigger: `receipt.json` deliberately does not
claim semantic verification. For application proofs, independently run the
existing `cargo xtask verify-native-relaunch-log` or `verify-elf-relaunch-log`
oracle. The corresponding `verify-*-relaunch-frame` command accepts a complete
canonical `P6\n1024 768\n255\n` RGB capture and checks the editor patch. Full-frame
comparison against a retained qualified capture is a separate check.

On the current AMD host, VirtualBox 7.2.6 boots the normal shell and workspace
desktop, but masks the `XSaveErPtr` capability required by the newer application
CPU profile. Both replacement proofs correctly stop at admission. Keep
using QEMU for that ABI proof until VirtualBox CPU-state preservation is
separately qualified; see the
[host investigation](../notebook/Development/2026-10-04%20Persistence%20Foundation%20and%20VirtualBox%20Log.md).

The portable durable-document foundation is in
`anomalous_document_protocol::durable`, under proposed
[ADR 0060](../docs/adr/0060-model-bounded-durable-document-recovery.md). It models
flush-before-acknowledgement, restart epochs and two-slot recovery through a
five-sector adapter. The separate `native_durable_test` profile now uses that
policy in userspace over a narrowly granted, feature-private ATA PIO mechanism.
`make native-durable-smoke` provisions a disposable disk, destroys both tasks,
and checks cold recovery through a new VM. Native command-failure cuts and
lost committed-reply reconciliation remain pending; the earlier RAM profiles
retain their original semantics.

The image builder is a Rust xtask, so each step is inspectable:

~~~sh
cargo xtask --help
~~~

## Repository map

| Path | Purpose |
| --- | --- |
| [boot/threshold](../boot/threshold) | Anomalous-owned BIOS and long-mode bootstrap |
| [kernel](../kernel) | Heap-free Rust kernel and bounded bootstrap memory core |
| [docs/architecture/kernel.md](../docs/architecture/kernel.md) | Implemented fault, circle, memory, and interrupt/input proofs |
| [crates/anomalous-abi](../crates/anomalous-abi) | Experimental control-plane wire layout |
| [crates/anomalous-session](../crates/anomalous-session) | Host-only bounded private shell-session ingress and waiting proof |
| [crates/anomalous-desktop-renderer](../crates/anomalous-desktop-renderer) | Fixed allocation-free semantic-capsule validator and software rasterizer |
| [vessels/anomalous-supervised-renderer](../vessels/anomalous-supervised-renderer) | Separately linked fixed-address renderer proof image |
| [crates/anomalous-desktop-theme](../crates/anomalous-desktop-theme) | Validated official themes and independent appearance components |
| [crates/anomalous-desktop-workspace](../crates/anomalous-desktop-workspace) | Canonical public windows, Appearance choices/selection, and granted human/machine commands |
| [crates/anomalous-desktop-compositor](../crates/anomalous-desktop-compositor) | Allocation-free RGB composition and shared window/Appearance control geometry |
| [vessels/anomalous-workspace-service](../vessels/anomalous-workspace-service) | Persistent owner, keyboard/pointer settings policy, bounded rejection recovery, and composition |
| [tools/xtask](../tools/xtask) | Reproducible image and VM-test orchestration |
| [eidolon](../eidolon) | Experimental lossless framebuffer-trace codec and Ny proving project |
| [oculus](../oculus) | Planned Matrix desktop system: session shell, window management, composition |
| [archon](../archon) | Planned Matrix scanning and security-assessment subsystem: findings, rules, analyzers |
| [optional runtime requirements](optional-runtime.md) | Generated, Git-ignored shared Ny runtime (compiler, interpreter, standard library) |
| [scripts/refresh-ny-runtime.sh](../scripts/refresh-ny-runtime.sh) | Build and activate the shared Ny runtime |
| [docs/architecture](../docs/architecture) | System, boot, and package architecture |
| [docs/adr](../docs/adr) | Architectural decision records |
| [docs/nomenclature.md](../docs/nomenclature.md) | Precise occult engineering vocabulary |
| [docs/roadmap.md](../docs/roadmap.md) | Capability-gated research roadmap |
| [docs/threat-model.md](../docs/threat-model.md) | Initial security boundaries and risks |
| [notebook](../notebook) | Working state, evidence, experiments, and research notes |

## Bootstrap shell and Canticle

The current ring-0 command loop is intentionally a bring-up instrument, not a
secure shell or stable language implementation. ADR 0020 now proves a fixed
ring-3 shell-vessel-shaped lifecycle, but that fixture has no parser, session,
machine encoding, authenticated semantic path, or physical input. The ring-0
interpreter has no files, network, scripts, variables, dynamic evaluation, or
privileged commands and will move out of the normal kernel path once Anomalous
can launch isolated vessels.

ADR 0021 separately accepts a bounded host model for private semantic input,
FIFO publication, checked session replacement, and lost-wake-safe waiting. That
model is not yet a real shell runtime. ADR 0022 accepts a removable x86-64
feature proof for one versioned nonempty reply and exact blocked-consumer wake,
but no production session transport or deployed router uses it. ADR 0023 accepts
a separate host-only router proof that consumes an external authentication
assertion; credential authentication, focus policy, physical-input routing,
production deployment, and normal-image integration remain open.

**Canticle** is the proposed native command language for the eventual
unprivileged shell vessel. Its human source and deterministic machine AST map
to the same typed program, with structured pipelines, explicit sigils,
preview-bound commits, typed errors, bounded evaluation, and governed secrets.
The current proposal deliberately excludes implicit interpolation, word
splitting, globbing, executable search, and general `eval`. See the
[Canticle architecture](../docs/architecture/canticle.md) and
[ADR 0010](../docs/adr/0010-canticle-secure-semantic-command-language.md).

## Compatibility objective

Running Linux, Windows, and macOS applications is a long-term research
objective, not a current capability or unconditional promise. The native kernel
and rite transport target is x86-64. The work will be tiered across source
portability, ABI or API personalities, translation, and deeply integrated
virtualization. Compatibility logic belongs in isolated user-space veils rather
than the kernel.

Linux is the most practical first personality for the planned TRI-ABI work.
macOS has its own Darwin/XNU foundation, combining Mach and BSD components;
it is not based on Linux. [Apple's XNU source](https://github.com/apple-oss-distributions/xnu)
describes that architecture. Native window buffers establish a rendering
foundation; executable loading, syscalls, runtime libraries, and foreign
window-system protocols still require their own contracts and evidence.

Required IA-32 machine-code
execution, including applications using the 32-bit Win32 ABI, will terminate in
a compatibility veil that translates foreign pointer widths, handles, PE/NT
object conventions, and calling conventions into the same fixed-width native
rites; it does not imply an IA-32 kernel. Windows will likely combine
user-space API translation with virtualization. macOS is
the hardest target technically and legally; any work must respect Apple
licensing, signing, and intellectual-property constraints.

## Native artifacts and packages

Anomalous will define its own artifact format for code plus governed context and
metadata. The first design step is the **Reliquary** (`.re`): a versioned, signed native
executable and package format capable of carrying execution entry records, code,
semantic schemas, governed context, requested sigils, provenance, SBOM data,
resource intent, and debug maps. Its first revision may embed conventional
machine code, but the framing and execution contract are Anomalous-native and
evolvable. Rich format parsing remains outside the kernel.

The `.re` extension and absence of fixed global native path-length/depth limits
are recorded in [ADR 0061](../docs/adr/0061-select-re-extension-and-path-policy.md).
The serialized Reliquary format and parser are still design work; current
native proofs load inspected segments, not `.re` files.

**Covenant** is the planned package and environment manager. Its contract is
declarative desired state, content-addressed storage, explicit capability
review, atomic activation, and rollback. Human and AI clients will review the
same transaction plan. See
[package-management.md](../docs/architecture/package-management.md).

## Eidolon and Ny

**Eidolon** is the proposed lossless framebuffer-trace codec under
[eidolon](../eidolon). It begins as host-side tooling for exact QEMU PPM captures,
with a dependency-free Python conformance oracle and a strict hostile-input
test suite. It is not linked into Threshold or the kernel and is not yet a
stable format.

The project is also the first production-shaped workload for Ny adoption.
Ny is the official language direction under
[ADR 0045](../docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md);
component migration remains evidence-gated. The Ny implementation is
deliberately gated on fixed-width bytes, real byte I/O, pointers, structured
errors, interpreted/native agreement, and a complete project runtime. See
[ADR 0016](../docs/adr/0016-adopt-ny-as-anomalous-implementation-language.md),
[ADR 0017](../docs/adr/0017-eidolon-lossless-framebuffer-traces.md), and the
[Ny feature ledger](../eidolon/docs/ny-feature-ledger.md). Existing verified Rust
boot and kernel code remains authoritative until qualified replacements meet
the migration gates. The language designation does not qualify Ny for a
privileged boundary or establish an Anomalous application runtime.

Anomalous keeps one project-wide, generated Ny runtime under `runtime/`, shared
by every sub-project. Refresh it from the sibling Ny checkout with
`scripts/refresh-ny-runtime.sh` (or `make -C eidolon sdk`); the runtime and its
private `.runtime/` generation store are Git-ignored. The current runtime is an
explicitly marked **development** SDK that carries the Matrix front end.

**Matrix** is a second surface language over the same NIL semantic core,
selected by the `.mat` (or `.matrix`) extension. **Oculus** is the planned
desktop system — session shell, window manager, compositor, workspace model,
input routing, and appearance — written in Matrix under [oculus](../oculus). It is
a scaffold today; its unit-by-unit plan is [oculus/roadmap.md](../oculus/roadmap.md).

**Archon** is the planned scanning and security-assessment subsystem, written
in Matrix under [archon](../archon). It is intended to become a core operating-
system facility that people and agents use to find security-relevant issues on
any surface — code, configuration, permissions, dependencies, or runtime state
— through one machine-readable finding model. It runs standalone today: the
`archon` command takes paths, picks a rule set by target kind (generic rules
plus a first Matrix-specific set), and prints findings and a machine-readable
report line. Its unit-by-unit plan is [archon/roadmap.md](../archon/roadmap.md).
Archon introduces a new platform-level abstraction, so operating-system
integration is gated on an accepted decision record and does not begin here.

## Design and code style

The project uses an occult naming scheme with formal, searchable definitions.
For example, a daemon or long-running service is a **vessel**. Names must clarify
architecture, not obscure it; first use in documentation includes the familiar
technical term.

Project-defined identifiers use snake_case, including Rust types and constants.
Rust and C-family code use K&R brace placement with four-space indentation.
External API identifiers retain their upstream spelling. Formatting is enforced
by rustfmt.

Neumorphism is a visual language, not an excuse to hide state. Depth and
material cues must be paired with contrast, focus outlines, text or icon state,
keyboard navigation, semantic accessibility, and reduced-motion behavior.

## Contributing and security

Read [CONTRIBUTING.md](../CONTRIBUTING.md) before proposing architectural changes.
Anomalous is pre-alpha; report security issues through the private process in
[SECURITY.md](../SECURITY.md).

Created by **Nathan Young** and developed with contributors.

## License

The open-source core is available under the [MIT License](../LICENSE), Copyright
(c) 2026 Nathan Young. Separate enterprise agreements may provide support,
certification, warranties, indemnity, managed distribution, and lifecycle
commitments without reducing the rights granted by the MIT license.
