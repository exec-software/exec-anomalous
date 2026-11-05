# ADR 0073: Run Linux Antimatter through the Linux veil

- Status: Accepted
- Date: 2026-10-07
- Scope: Product direction; Linux application support remains open
- Supersedes: ADR 0045's expectation of a native Antimatter frontend for the first application milestone
- Related: ADR 0008, ADR 0045, ADR 0050, ADR 0051, ADR 0052, ADR 0053, ADR 0072

## Context

After trying ADR 0072's native reference editor, the user clarified the intended
application target: Antimatter already ships for Linux and should run in that
existing form through Anomalous's ELF/Linux backend. A port of its frontend or
runtime to the native ABI or Reliquary format is not required. The earlier
native-first sequence establishes the operating-system baseline; it does not
make a rewritten Antimatter the application deliverable.

The reference editor is a separate, small Rust application. Its native ownership,
input, presentation, copied transport and save/recovery evidence remains useful,
but additional fixture behavior cannot establish that Antimatter runs. ELF
container parsing alone also cannot establish Linux binary compatibility.

## Decision

1. **Use the existing Linux Antimatter application as the first real application
   target.** Preserve its Electron/React/CodeMirror frontend and application
   behavior. Anomalous must supply the Linux environment its shipped executable
   and dependencies require. Do not make a native frontend rewrite, Ny port or
   `.re` conversion a prerequisite for this milestone.
2. **Develop the Linux execution personality above native mechanisms.** Continue
   the supervised userspace-veil direction of ADRs 0008 and 0053. ELF admission,
   Linux startup, syscall policy, library loading and desktop integration have
   separate responsibilities. Linux policy must not acquire ambient authority
   or be distributed through unrelated kernel paths.
3. **Keep the native reference as a regression baseline.** Native resource
   ownership, recovery and cleanup constrain compatibility implementations.
   Qualify Linux behavior with ordinary Linux binaries, progressively approaching
   the actual application and reporting unsupported behavior.
4. **Drive prerequisites from the target artifact.** Inspect the exact Linux
   executable, interpreter, libraries, resources and native modules. Follow with
   isolated host reference traces where needed. Separate packaging requirements
   such as an AppImage wrapper from the contained application's requirements;
   do not change application logic to conceal a missing compatibility operation.
5. **Preserve the broader direction.** Antimatter remains the official editor,
   Ny remains the official language direction, and HAG remains the compute
   direction. Reliquary `.re` remains the native format direction. Windows and
   macOS compatibility follow Linux with their own personalities and evidence.

## Current boundary and implementation sequence

ADR 0051 admits only a fixed, sectionless static ELF64 image, at most 64 KiB,
with two exact load segments and no interpreter, dynamic linking or TLS.
ADRs 0052/0053 implement read, write, poll, yield and exit for two preopened
Anomalous record devices. They do not implement a Linux filesystem, standard
process environment or general Linux application support. ADR 0072's native
desktop integration does not remove these restrictions.

The next target is a small, unmodified dynamically linked Linux executable,
such as a pinned distro `/usr/bin/true`, with its matching interpreter and libc
in a private packaged filesystem view. It must enter through the Linux process
ABI and exit correctly without recompilation or Anomalous-specific calls.
This is an implementation target, not a qualified capability. It requires owned
multi-segment/PIE and interpreter admission, Linux initial process state, memory
mapping/protection, TLS and the startup services its actual loader requests.
The precise subset and decomposition require artifact inspection and traces.

Subsequent Linux binaries should exercise standard file/descriptor behavior,
processes/threads, synchronization, signals and events before the selected
Electron build. Provide its selected Linux window/display/input path backed by
Anomalous desktop services, then launch the existing Antimatter bundle. Software
rendering may precede GPU/HAG acceleration. These are planned dependencies, not
a claim that the complete application is one loader patch away. New execution
boundaries still require their own design and qualification. Builds and VMs
remain serialized and memory guarded; no Chromium source build is implied.

## Alternatives and consequences

A native Antimatter frontend rewrite is unnecessary for this target. More native
editor fixtures may still answer a specific mechanism question, but they must
not displace real Linux executable progress or be reported as the product port.
The compatibility work belongs primarily to Anomalous; upstream Antimatter
changes should address an identified application issue, not disguise missing
Linux behavior. Existing accepted language and native-format directions remain.

## Acceptance

Product Milestone 1 requires the actual Linux Antimatter build to launch and
render its real interface inside Anomalous, with input reaching that application.
The useful-editor follow-up opens a real file, edits it, saves it, exits and
reopens the exact saved bytes. Compare appearance and behavior with the same
Linux build on the host. Record executable and dependency identities,
unsupported features, compatibility tier, memory use, failures and cleanup.

A native demonstration, a successful ELF parse or a small Linux test program is
prerequisite evidence. Report each as such. This decision selects the target and
does not claim that Antimatter or Electron currently runs in Anomalous.
