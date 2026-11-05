# ADR 0045: Select Antimatter, Ny, and HAG for the native userspace direction

- Status: Accepted
- Date: 2026-10-03
- Scope: Product direction; implementation gates remain open.

## Clarified application target — 2026-10-07

Accepted [ADR 0073](0073-run-linux-antimatter-through-the-linux-veil.md) records
the user's resolved target: run the existing Linux Antimatter build through
Anomalous's ELF/Linux compatibility backend. A native Antimatter frontend or
`.re` port is not required. Native-first means proving the native ABI baseline
before foreign compatibility; the small native reference is not the intended
Antimatter deliverable. This supersedes the frontend expectation and unresolved
frontend-choice wording below, while preserving the official editor, Ny/HAG
directions and historical qualification evidence.

## Resolved baseline order — 2026-10-03

The user has now selected the native ABI baseline first, a separately qualified
ELF path next, then comparison with the Linux copy of Antimatter running on the
host outside the Anomalous VM. Windows and macOS comparisons follow later.
The first native service proof uses a bounded Rust application client and does
not claim to port Electron, the full Antimatter frontend, or Ny's hosted runtime.
Proposed [ADR 0050](0050-prove-a-bounded-native-application-contract.md) records
that experimental application contract and independent-client scope. Ny remains
the official language direction; a qualified Rust proof does not reverse it.

The native baseline compares document/selection/undo/appearance behavior,
explicit surface/input ownership, and lifecycle. A host reference is a behavioral
control, not a guest Linux or Electron compatibility claim. ELF admission must
name its execution personality separately. The earlier accepted direction and
the interim clarification below remain historical context; they do not require
full Antimatter or Electron compatibility before this bounded ABI proof.

## Interim clarification after the bounded Ny-object proof — 2026-10-03

At this intermediate point, the user's priority was to establish the native application ABI baseline
first, then compare Windows, macOS, and ELF-based execution against that common
baseline. Antimatter is largely Electron today. Its first real multi-ABI
application test must account for that runtime and service dependency; the
choice between preserving Electron/React through a runtime port and a limited
native frontend was undecided. The earlier decision text below records the
accepted direction, but its statement that Electron/Chromium is not a
prerequisite must not be read as a newly confirmed frontend strategy.

Executable containers and execution/service ABIs are separate: the selected
Windows PE/COFF and macOS Mach-O profiles need their respective Windows and
Darwin personalities; an ELF profile needs its own named personality rather
than an inferred Linux contract.
The qualified Ny ELF object is a native build input, not Linux compatibility.
Future comparison should exercise the same application behavior, task/resource
ownership, surface/input authority, service results, and fault/exit cleanup,
with unsupported behavior stated for each profile. This clarification freezes
no wire fields and claims no implemented foreign adapter or Electron port.

This record supersedes the primary-language designation in
[ADR 0003](0003-rust-assembly-and-code-style.md). Its style and narrow,
documented unsafe/assembly requirements remain in force. The technical adoption
and privileged-migration gates in proposed
[ADR 0016](0016-adopt-ny-as-anomalous-implementation-language.md) remain open.

## Context

The user selected Antimatter as Anomalous's official editor, Ny as the official
language for Anomalous and HAG, and HAG as the compute runtime being developed
to replace Vulkan. After the core kernel is ready for a bounded application
profile, the next major slice is a native Antimatter build running and rendering
in Anomalous userspace.

These projects currently have different host dependencies. Antimatter's desktop
frontend uses Electron, React, and CodeMirror; Ny's qualified native release
profile targets Linux and depends on its host runtime; HAG's experimental device
transport uses Linux AMDGPU. None currently provides an Anomalous application
port. The dated
[direction log](../../notebook/Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
records the inspected source and gaps.

## Decision

1. **Antimatter is the official editor.** Its first native Anomalous milestone
   is an independent userspace application with its own editing state and
   rendered surface. The native frontend should carry forward Antimatter's
   document, command, appearance, and save behavior. The existing browser
   renderer is not a portable native library; its port is explicit work.
2. **Ny is the official language direction for Anomalous and HAG.** Qualify a
   bounded Anomalous userspace target/runtime for the native editor. Existing
   verified Rust remains authoritative until each replacement has independent
   evidence. This designation neither rewrites HAG nor authorizes an unqualified
   kernel/Threshold migration. Privileged migration still requires its own ADR.
3. **HAG owns compute execution and resource lifetimes.** It is intended to
   replace Vulkan in this stack. Model and agent semantics remain above HAG;
   HAG does not replace the shared semantic control plane, confer authority, or
   by itself provide windowing, text rendering, or display presentation. Its
   Anomalous transport needs separately qualified memory, device, submission,
   completion, cancellation, and retirement mechanisms.
4. **Native userspace comes before foreign compatibility.** Finish the selected
   kernel/application gates, then pursue the native Antimatter milestone. Linux
   remains the first foreign execution target, with macOS following separately.
   Electron/Chromium compatibility is not a prerequisite for the native editor.
5. **Prove a small editor before expanding it.** Begin with host cross-compilation,
   a bounded runtime, and a software reference rendering path. First prove launch,
   a visible text surface, input, and cleanup; then open/edit/save/relaunch with
   exact byte verification. HAG acceleration, local inference, a guest compiler,
   terminals, and broad editor parity are separate increments.

The immediate kernel slice remains the retained prepared-launch owner described
in the [execution-readiness audit](../architecture/execution-readiness.md):
explicit commit, retryable cancellation of partial preparation and unstarted
ready tasks, failure injection, and exact restoration. An accepted direction
does not make that launcher or an application ABI implemented.

## Consequences

Kernel readiness is evaluated against an explicit first application profile,
including sustained dispatch, supported CPU state, checked copies, task-bound
input/surface authority, resource budgets, and fault/exit cleanup. Filesystem and
editor policy belong in userspace services. ABI fields, transport formats, and
new privilege boundaries need their own decisions and evidence.

The first useful editing acceptance is a native application opening a bounded
`.ny` fixture, accepting text/caret/selection/scroll operations, saving through
an explicit workspace service, and recovering the saved bytes after application
relaunch. A frame alone establishes rendering progress, not editor completion;
a mocked or same-vessel window does not establish independent application
isolation. Durable storage across OS reboot is a later, separately stated gate.

Build concurrency and memory limits stay explicit. Record guest and host peak
memory, fault cleanup, and update timing before increasing workloads. Do not
load inference models or run GPU stress merely to prove the editor surface.

## Alternatives

- Porting Electron/Chromium first would preserve more frontend source but make
  extensive browser, Node, process, filesystem, and graphics compatibility
  prerequisites for the first native application.
- A wholesale rewrite of proven Rust would postpone the userspace milestone
  and replace reviewed evidence before Ny's target/runtime is qualified.
- Waiting for complete HAG acceleration or full inference would couple editor
  bring-up to unrelated device and model qualification.
- Continuing directly to Linux compatibility would defer the user's selected
  native product milestone.
