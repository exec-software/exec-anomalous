# Native Antimatter direction

Date: 2026-10-03

## Resolved application target — 2026-10-07

The user tried the native reference and clarified that Antimatter can ship in
its existing Linux form. The native-first work establishes an ABI baseline;
it does not require a native Antimatter frontend or `.re` port. The first real
application target is the existing Linux Antimatter build running through the
ELF/Linux compatibility backend. Accepted
[ADR 0073](../../docs/adr/0073-run-linux-antimatter-through-the-linux-veil.md)
supersedes the earlier unresolved frontend choice and native-app expectation.
The application UI and logic should remain shared with the working Linux build.

Read-only inspection found Antimatter clean at
`23b92422f87ae063c02fa07cb8f4ec5c35922754`, version 9.0.0. Its installed Electron
44.3.0 executable is 228,130,120 bytes: ET_DYN/PIE, four LOAD segments, twelve
program headers, `/lib64/ld-linux-x86-64.so.2` as interpreter, TLS, dynamic
relocations and 34 direct shared-library dependencies. These include glibc, GTK,
X11, GBM, NSS, D-Bus and ALSA. Static imports identify memory mapping/protection,
thread/process, signal, socket, event and shared-memory needs. This is not a
measured syscall trace or a claim that every imported feature is used at startup.

Anomalous currently admits only a fixed sectionless ET_EXEC profile with two
LOADs and a 64 KiB file bound. Its Linux policy supports five calls against
custom preopened record devices, with no dynamic-loader/TLS or general Linux
file/process/display environment. The next compatibility target is an unchanged,
pinned distro `/usr/bin/true` with matching interpreter and libc, followed by a
stock command using arguments/stdout. The local `true` is 35,288 bytes but already
requires PIE, four LOADs, interpreter startup and libc, directly exercising
dependencies relevant to Electron. This target has not booted in Anomalous.

Artifact hashes and raw readelf headers/program headers/dynamic entries are
retained in
an external validation directory (not distributed).
The inspection executed no target binary, build, install or VM. Antimatter source
and installed profiles were not changed. The sections below preserve the
historical decisions and native prerequisite evidence.

## Selected destination

The user selected Antimatter as Anomalous's official editor, Ny as the official
language for Anomalous and HAG, and HAG as the compute runtime being developed
to replace Vulkan. Native Antimatter running and rendering in userspace is the
next major slice after the core kernel can support its bounded profile.
[ADR 0045](../../docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md)
records this direction. It does not claim the ports or adoption gates are done.

## Displacer follow-up direction

The user also identified Displacer as a potential shared foundation for the
KDE Plasma-style desktop and customization. Carry the existing Anomalous theme
into that direction and contribute useful changes back where appropriate.
This is a future integration note, not an instruction to inspect or change
Displacer during the native/ELF qualification slice. No Displacer work has been
performed as part of this note.

## Resolved order — native first, ELF next, then comparison

The user resolved the immediate scope: prove the native application ABI first,
qualify an ELF path next, then compare against the Linux Antimatter copy running
on the host outside the VM. Windows and macOS comparisons follow later. Proposed
[ADR 0050](../../docs/adr/0050-prove-a-bounded-native-application-contract.md)
uses a bounded independent Rust application client for the first contract.
It exercises shared document/selection/undo/appearance semantics and task-bound
surface/input ownership. It is not a full Electron/Antimatter or Ny runtime port,
and it does not change Ny's official-language direction.

The host Linux copy provides reference behavior; it is not a guest compatibility
result. ELF parsing and a selected execution personality need separate evidence.
Later Windows PE/COFF and macOS Mach-O profiles must provide their corresponding
personalities above the same native authority and service boundaries. No loader
or runtime is selected merely by its container format. The interim question and
four-test reference run below record the preceding planning step.

## Interim clarification — native ABI baseline and multi-ABI comparison

After the bounded Ny-object proof, the user emphasized that Antimatter is largely
Electron and that the native ABI baseline comes first. Antimatter is intended
to become the first real application comparison across that native path and
eventual Windows, macOS, and ELF profiles. The existing accepted ADR 0045 text
said Electron/Chromium compatibility was not a prerequisite; its dated
clarification now records that preserving Electron/React through a runtime port
versus choosing a limited native frontend was then undecided. The original
audit and candidate slices below are historical planning evidence, not approval
to rewrite the frontend.

PE/COFF, Mach-O, and ELF describe executable containers. The corresponding
Windows, Darwin, or explicitly selected ELF execution personality must also
define startup, calling/runtime requirements, and service behavior. ELF does
not itself select Linux: ADR 0049's qualified Ny object uses ELF as a native
build input and supplies no Linux personality. Reliquary framing and Covenant
packaging remain separate proposed work.

The next service proof should establish task-bound surface/input ownership and
revocation on the native path without requiring a frontend decision. Later
comparison acceptance should run matching application operations and examine
rendered and semantic state, input targeting, workspace results, resource
ownership, and fault/exit cleanup. Every profile must name unsupported behavior
and runtime dependencies. This is a conceptual comparison contract, not new
wire fields, completed foreign adapters, Electron compatibility, or a native
Antimatter implementation.

While frontend scope remained open, a bounded host reference run built only
Antimatter's existing `test/editor-terminal-theme.test.ts` with its installed
esbuild and passed all four original Node tests: terminal-theme contrast,
terminal-state preservation, CodeMirror document/selection/undo invariance
across theme changes, and cached two-document appearance. Ten selected
source/configuration files remained hash-identical. The scope peaked at
86,040,576 bytes (about 82.05 MiB) under a 4 GiB soft/6 GiB hard memory limit
and 512 MiB swap limit; host available memory stayed above 23,791,468,544 bytes.
All memory-event and OOM counters were zero. Evidence is in
an external validation directory (not distributed), including
`reference-results.json`, `reference-memory-summary.json`, source hashes,
the build metafile, and logs. This is host behavioral reference evidence;
it proves neither an Electron boot on Anomalous nor native editor execution.
That clarification and reference follow-up changed no kernel, native payload,
or Antimatter source code.

## Earlier implementation follow-up

Implementation follow-up: the first proposed slice below is now implemented and
validated under [ADR 0046](../../docs/adr/0046-prove-retained-prepared-launch-ownership.md).
The [Prepared Launch Ownership Log](2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records that bounded result; scheduling and application/runtime gates remain.

## Read-only sibling audit

Inspected the current working checkouts under an external validation directory (not distributed)
on 2026-10-03. Base commits were Antimatter `0679c44`, Ny `1ea16a3`, and HAG
`a7cf191`; the active working trees also contain ongoing work. These observations
describe those working trees, not reproducible clean-commit release results.
No sibling edits, builds, installations, model loads, or GPU work were performed.

| Project | Observed implementation | Anomalous gap |
| --- | --- | --- |
| Antimatter | Electron 44.3, React 18/TypeScript/Vite, CodeMirror 6, Electron IPC and Node file operations; separate xterm/node-pty terminal. | DOM/CSS renderer needs an admitted runtime/rendering path, event/file/service bridge, and executable profile. Runtime preservation versus a limited native frontend remains undecided. |
| Ny | `1.0.0-rc.1`; host-native LLVM generation with a C-shaped runtime ABI and C++17 runtime; qualified Linux x86-64 profile. | Explicit Anomalous target/layout, entry and linker profile, bounded runtime allocation/failure behavior, service bindings and lifecycle conformance. The current release excludes freestanding/Anomalous. |
| HAG | Rust `std` library; Linux AMDGPU transport and bounded experimental compute fixtures. General native compute remains unavailable. | Separate Anomalous device transport and resource/submission lifetime qualification. Compute access supplies neither the editor renderer nor a display protocol. |

Source anchors, relative to the named sibling repositories:

- Antimatter: `package.json`, `renderer/src/environment/CodeEditor.tsx`,
  `renderer/src/environment/store.ts`, `renderer/src/environment/highlight.ts`,
  `src/preload/index.ts`, `src/main/environment/files.ts`, and
  `docs/native-beholdd-transition.md`. The transition document already selects
  Antimatter for Anomalous and calls for Beholdd embedding without Electron/HTTP.
  Existing document/save/conflict contracts and design assets can guide the
  native port; TypeScript code requires a compatible runtime or an explicit port.
- Ny: `docs/release/v1-profile.md`, `code/nycompiler/src/llvm_backend.cpp`,
  `native_runtime.hpp`, `native_runtime.cpp`, `parallel_support.hpp`,
  `secure_file_io.cpp`, `time_support.hpp` in that source directory, and
  `code/nylinker/main.cpp`. LLVM currently selects the host target; generated
  startup uses `main`, and the host linker adds pthread support. A Linux binary
  cannot be relabeled as an Anomalous native application.
- HAG: `docs/native-backend.md`, `docs/native-compute.md`, `src/lib.rs`, and
  `src/linux/compute/packets.rs`. The sibling direction assigns hardware
  execution/lifetimes to HAG and model/inference semantics to Beholdd. Its
  render-node access is a compute transport, not desktop presentation.

The September 9 Ny inventory in ADR 0016 is historical. Current host language
features have advanced; that does not close the separate Anomalous runtime and
privileged-migration gates.

## Proposed implementation slices

These are dependency and acceptance candidates, not newly frozen ABIs.

1. **Retain prepared launch ownership.** Continue the three-region GPR-only
   kernel proof with explicit commit and retryable cancellation. Inject every
   preparation failure, including cancellation of an unstarted ready task, and
   require exact resource restoration before the successful ring-3 exit case.
2. **Qualify the first application execution profile.** Integrate owned roots
   with sustained dispatch, checked copies, task-local faults and exit/restart.
   State CPU/TLS/FPU limits explicitly and inspect generated code against them.
   Prove a tiny cross-compiled Ny userspace program with bounded runtime/startup
   and attributable failure. Host target success alone does not qualify it.
3. **Qualify a separate application surface client.** Provide task-bound revocable
   surface/input grants, focus routing, events/waits, diagnostics, allocation
   budgets and cleanup. A bounded copied-frame proof can establish the native
   service boundary while Antimatter's runtime/frontend scope remains open;
   use software rendering as a reference. The existing workspace
   content producer shares a trusted vessel with its compositor and does not
   yet satisfy this boundary.
4. **Close the first editing loop.** Start with a bundled `.ny` fixture and a
   narrowly authorized workspace service. Add text insertion/deletion, caret,
   selection, scrolling, dirty state, conflict-aware save, and app relaunch.
   Specify whether backing storage survives application restart or OS reboot;
   only claim the persistence actually tested.
5. **Grow native integration independently.** Ny highlighting/compiler services,
   project navigation, terminal/process services, Beholdd embedding, and HAG
   acceleration follow as bounded increments. HAG's device contract must cover
   DMA isolation, budgets, safe completion/cancellation, and uncertain-lifetime
   retention before general device work. Ny adoption in HAG is separate from
   porting its existing Rust transport; neither is implied by a designation.

The narrow native contract can begin with a fixed build artifact and inert
in-memory load plan. A serialized Reliquary envelope and general package loader
need their own gates. Full Covenant, networking, inference, and a compiler
running inside the guest need not block the service proof. Dependencies for
the first Antimatter frame depend on the pending runtime/frontend decision.

## First milestone evidence

Rendering checkpoint: boot Anomalous, launch the native Antimatter executable
as an independent task, render text and a caret in its granted surface, accept
input, exit, and reclaim its allocations. Report the actual implementation
language and runtime; a scaffold is not evidence of the qualified Ny port.

Useful-editor checkpoint: open the bounded Ny fixture, execute a scripted edit,
save, exit and relaunch, and read back the exact expected bytes. Capture a
deterministic frame for text/caret/selection/clipping; expose matching semantic
state for nonvisual clients. Exercise stale focus/surface grants, injected app
faults, repeated launch/exit, and save failures without corrupting prior content
or stopping peer/compositor progress.

Record peak guest allocations, host scope memory, OOM counters, and frame/update
timing under an explicit profile. Begin host builds with two jobs, sequential
heavy checks, an 8 GiB hard scope ceiling and a 4 GiB available-memory stop
threshold, as in the recovery run. The interactive VM has a separate 2 GiB
scope ceiling. Recheck current headroom before each heavy run; other development
sessions share the machine. This documentation pass required no heavy runs.

## Remaining design questions

- Smallest qualified Ny runtime subset and supported CPU state for the first app.
- Preservation of Electron/React through a runtime port versus a limited native
  frontend, including source ownership and shared commands/design assets.
- Text/font coverage, layout and input-method tier for the first acceptance case.
- Versioned surface/input/file contracts, event backpressure and revocation.
- Initial save-service backing and explicit crash/durability guarantees.
- HAG device/familiar boundary and independent hardware qualification plan.

Resolve these at the relevant bounded slice rather than treating this direction
record as an application ABI or a promise of full editor feature parity.
