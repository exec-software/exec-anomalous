# Development environment recovery

Date: 2026-10-03

## Recovered source and host

Validation resumed on a replacement Linux host from the preceding September
checkpoint. All 370 initial tracked file hashes were unchanged before the
QEMU runner correction below. Machine-specific locations are not part of the
public evidence; the tool versions and observed behavior are retained here.

The host is Ubuntu 26.04.1 LTS on x86-64 Linux 7.0.0-34-generic, with QEMU
10.2.1, GNU Make 4.4.1, and GNU binutils 2.46. The repository's pinned
`nightly-2026-05-07` toolchain was installed with `clippy`,
`llvm-tools-preview`, `rust-src`, `rustfmt`, and `x86_64-unknown-none`.
It reports `rustc 1.97.0-nightly (365c0e1d7 2026-05-06)` and
`cargo 1.97.0-nightly (4f9b52075 2026-05-01)`.

## QEMU proof runner correction

The initial proof runs emitted expected guest success records but timed out
instead of terminating. QEMU's
[`isa-debug-exit` implementation](https://gitlab.com/qemu-project/qemu/-/raw/v10.2.1/hw/misc/debugexit.c)
requests guest shutdown with the encoded exit status. The runners' inherited
`-no-shutdown` policy instead pauses the VM on guest shutdown, as described in
the [official invocation documentation](https://www.qemu.org/docs/master/system/invocation.html).
This was observed on this host with QEMU 10.2.1; no first-affected release is
claimed.

The correction in `tools/xtask/src/main.rs` lets the common proof command
builder terminate on guest shutdown, removes the semantic transport override
and standalone interrupt runner's pause policy, and explicitly retains
`-no-shutdown` at the three interactive callers of the common builder.
The generic process runner, deadlines, exact exit-status checks, and transcript
verifiers are unchanged. Standalone frame-capture, desktop, workspace, and
shell runners retain their existing monitor-controlled lifecycles.
Independent review checked every common-builder caller and found no remaining
issue in this change.

## Saved resume-flag correction

The first matrix after fixing QEMU termination passed 19 of 21 targets.
`input-wait-smoke` rejected an expected exception with process status 35;
`supervised-renderer-smoke` timed out waiting for its successful completion.
Retained images, ELF symbols, QEMU traces, and debugger captures established
that both exact expected UD2 faults had the architectural resume flag (RF,
bit 16) set in their saved flags:

| Fixture | Vector/error | RIP | CS | Saved flags | RSP | SS |
| --- | --- | --- | --- | --- | --- | --- |
| Input wait | 6 / 0 | `0x7000000010e1` | `0x33` | `0x10002` | `0x700000006000` | `0x2b` |
| Supervised renderer | 6 / 0 | `0x6000000010f0` | `0x33` | `0x10046` | `0x600000094000` | `0x2b` |

[Intel SDM Volume 3B, §18.3.1.1](https://cdrdv2-public.intel.com/671427/253669-sdm-vol-3b.pdf)
specifies that the saved flags for fault-class exceptions carry RF, with the
stated instruction-breakpoint exception. The fixtures' exact flag predicates
had omitted that saved-state possibility. Their bounded correction admits only
each existing exact flags value or that value with RF set, retaining all task,
fault-address, selector, stack, and other flag requirements. It reuses the
existing scheduler `resume_flag` constant. Independent review confirmed exactly
`0x2`/`0x10002` for input wait and `0x46`/`0x10046` for the renderer, without
relaxing any other flag or exception predicate.
The targeted native reruns passed: input wait in 43.51 seconds and supervised
renderer in 45.61 seconds. Their logs are `input-wait-smoke-rf-fix.log` and
`supervised-renderer-smoke-rf-fix.log` in the recovery evidence directory.
Saved-frame captures and retained images/symbols are in
`input-wait-smoke-diagnostic/` and
`supervised-renderer-smoke-diagnostic/`.

## Fresh validation

The baseline `make check` passed before the runner change. After both reviewed
corrections, all **21 final TCG validation targets passed** in 978.69 seconds of
combined target time. This includes the complete `make check` (306.12 seconds)
and these twenty VM/image targets, run sequentially:

```text
smoke                         fault-smoke
circle-smoke                  control-smoke
rite-smoke                    dispatcher-smoke
scheduler-smoke               supervision-smoke
shell-vessel-smoke            memory-smoke
interrupt-smoke               input-wait-smoke
shell-smoke                   display-vessel-smoke
supervised-renderer-smoke     semantic-surface-transport-smoke
semantic-presentation-smoke   desktop-smoke
workspace-smoke               normal-exclusion
```

The final input-wait and supervised-renderer proofs passed in 30.02 and
30.41 seconds. Workspace smoke passed in 71.13 seconds, including its strict
frame and input checks. Both `make image` and `make workspace-image` were
rerun after the matrix, restoring the normal and native-workspace images.
A post-matrix comparison found all 370 files identical to the reviewed code
freeze; no code changed while this final matrix ran.
The earlier September 27 evidence and counts remain unchanged in their own log.

### Accelerated repeats and memory bounds

The three additional KVM targets passed: `memory-smoke` in 32.51 seconds,
`dispatcher-smoke` in 31.51 seconds, and `workspace-smoke` in 48.51 seconds.
This completes **24 passing validation targets**: the 21-target TCG matrix plus
three KVM repeats, totaling 1,091.22 seconds of combined target time.
The recovery-only wrapper `kvm-bin/qemu-system-x86_64` forced
`q35,accel=kvm` with no fallback; `kvm-invocations.jsonl` records all ten QEMU
invocations, including the memory proof at 96 MiB and 320 MiB. The host KVM
API check returned version 12. The wrapper lives in the recovery evidence
directory and is not a repository or system QEMU change.

In response to the reported earlier memory-pressure concern, these KVM repeats
used `CARGO_BUILD_JOBS=2` and ran sequentially inside the transient
`anomalous-kvm-validation.scope`, with `MemoryHigh=6GiB`, `MemoryMax=8GiB`,
and `MemorySwapMax=1GiB`. A two-second sampler checked host available memory,
host swap use, cgroup peak usage and memory events. Its guard would stop only
this validation task if available host memory fell below 4 GiB or a new OOM
kill appeared; the guard did not trigger.

The minimum observed host available memory was 15.52 GiB. The validation scope
peaked at 1.54 GiB, while total host swap use peaked at about 513 MiB. The
host OOM-kill counter remained 0 before and after the run; every scope memory
event counter, including `high`, `max`, `oom`, and `oom_kill`, remained 0.
`memory-watch.jsonl` and `memory-watch-summary.json` retain these measurements.
They describe this monitored run and do not establish a cause for an incident
in another session.

`kvm-validation-results.json` records the three passes.
`source-validation-after-kvm.json` confirms that all 370 frozen files still
matched after the accelerated runs. Normal and workspace images were restored
again afterward; `images-after-kvm.log` records that final restoration.
The two restored images match the exact SHA-256 values recorded for the
September 27 checkpoint; this comparison makes no claim about other proof
images. `final-image-sha256.json` records:

| Restored image | SHA-256 |
| --- | --- |
| `build/anomalous.img` | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| `build/workspace-dev.img` | `65d6cfe39038ac6c078139e9d052a3f790f31eb8b1a018ebd3243cf904251077` |


Evidence is retained under
an external validation directory (not distributed).
The initial failed runs are separated in `baseline-qemu-exit/`;
`validation-results.json` and per-target logs record the corrected runs.
`source-before.json` and `source-validation-initial.json` record the initial
source identity and comparison. `matrix-before-rf-fix/` preserves the first
19/21 result. The final code freeze in `source-final-code.json` retains all 370
baseline files with exactly three changed files: the xtask runner and the two
feature fixtures above; no baseline file is missing.
`source-validation-after-matrix.json` records that every frozen file remained
unchanged through the final matrix, before this notebook update.
The final verified `build/workspace_initial.ppm` was converted losslessly to
an external validation directory (not distributed);
`workspace-preview-provenance.json` records identical RGB pixels at 1024×768.

## Eidolon and the local Ny SDK

The Ny SDK was rebuilt from a clean isolated checkout of
`1ea16a3bbbe84c33a7293cbd828478581d16ab4b` under
`eidolon/build/ny-source`. Concurrent changes in the sibling Ny checkout were
preserved and excluded from this SDK. Installed LLVM 21 exposed unsupported
LLVM API differences, so official Ubuntu LLVM 18.1.8 packages were unpacked
locally under `eidolon/build/llvm18/root`, without changing system packages.
A build flag supplies the standard `<algorithm>` header needed by the host
C++ library without editing Ny source.

The local recovery wrapper is `eidolon/build/refresh-local-sdk.sh`. It uses
the clean source, local LLVM CMake package, a Python 3.12.14 runtime, and
`eidolon/build/tmp` for temporary files. The Python version preserves the
qualified JSON refusal behavior; the repository filesystem avoids the
submillisecond timestamp behavior observed on `/tmp`. SDK qualification
passed all 370 CTests in 81.32 seconds and activated
`eidolon/sdk -> .sdk/versions/456a9a1113d74f6f1c0bcf310cdcd9dc34ead774a584a22770ad7542c4cd676e`,
containing eight tools, fifteen modules, and twenty-seven documents.
LLVM is linked statically; the active SDK does not require an
`LD_LIBRARY_PATH` override.

Eidolon consumer checks passed with ordinary system Python 3.14.4 and no skips:
`make -C eidolon check` passed the 61-test suite, the separate 26-test Ny suite,
Python compilation, and shell syntax checks. The six native-specific tests
passed, and `ny-native-cli` produced textual/binary NIL plus the standalone
`eidolon/build/ny/eidolon`. The commands used from the repository root were:

```sh
env -u LD_LIBRARY_PATH make -C eidolon check
env -u LD_LIBRARY_PATH make -C eidolon check-ny-native ny-native-cli
```

The local SDK can be refreshed with:

```sh
env -u LD_LIBRARY_PATH eidolon/build/refresh-local-sdk.sh
```

The Python 3.12 requirement applies to the SDK's own qualification suite,
not ordinary Eidolon use. Logs are `eidolon-sdk-final.log`, `eidolon-check.log`,
and `eidolon-native.log` in the recovery evidence directory.

The wrapper, isolated source, LLVM packages, SDK generations, and builds are
ignored local development artifacts. Preserve them if moving this environment;
the wrapper's default Python path is specific to this machine and can be
overridden with `NY_PYTHON_BIN`.

## Resume interactive development

```sh
cd /path/to/exec-anomalous
CARGO_BUILD_JOBS=2 make workspace-run
```

This is the current desktop path. It builds `build/workspace-dev.img` and
opens **Anomalous Workspace Development**. QEMU defaults to automatic KVM
selection with TCG fallback; `ANOMALOUS_QEMU_ACCEL=tcg make workspace-run`
forces software emulation. Each launch has a private disk copy and logs under
`build/desktop-runs/`, allowing images to be rebuilt while the VM remains open.
`make workspace-image` builds without launching; `make run` opens the normal
bootstrap image, and `make desktop-run` opens the earlier desktop launcher.

An initial graphical launch with the explicit TCG command reached boot,
workspace-pointer, and workspace-service readiness and emitted three
`frame_presented` records. Its private logs are in
`build/desktop-runs/run-3-0/`. It used an independent image copy while the
regression matrix rebuilt shared artifacts. A later session poll returned exit
status 0; the final host inventory found no remaining process from that launch.
The final launch used:

```sh
CARGO_BUILD_JOBS=2 ANOMALOUS_QEMU_ACCEL=kvm make -j1 workspace-run
```

It is running in the active transient `anomalous-workspace-development.scope`
with a 2 GiB memory limit. At the readiness check it used about 109 MiB and
had emitted `boot.ready`, `workspace_pointer.ready`, `workspace_service.ready`,
and `workspace_service.frame_presented`. Its private log is
`build/desktop-runs/run-79820-0/qemu_desktop_dev.log`;
`kvm-gui-summary.json` records the active unit and measured memory.
This launch explicitly requires KVM and does not fall back to TCG.

| Control | Action |
| --- | --- |
| Mouse | Focus, drag titlebars, resize from the lower-right corner, and use window/panel controls |
| Tab | Focus the next visible window |
| P | Open or restore Appearance |
| F2 / F3 | Open Graphics Lab / switch its scene while focused |
| X / T | Maximize or restore / cycle floating and tiled layouts |
| 1 / 2 / W | Select workspace / move the focused window to the other workspace |
| Esc | Return to the bootstrap shell |
| `desktop` at the shell | Start a fresh workspace, discarding the preceding window/settings state |
| Ctrl+Alt+G | Release QEMU keyboard/mouse capture |
| Close the QEMU window | Stop the VM |

The desktop's Appearance, Graphics Lab, and public demo windows remain built-in
content in one trusted ring-3 vessel, rather than independently launched
application processes.

## Development checkpoint and next slice

The current kernel increment remains Proposed
[ADR 0044](../../docs/adr/0044-prove-owned-low-address-user-execution.md): two
GPR-only ring-3 tasks using separate owned code/data/guarded-stack mappings,
four resumed private reports, B's contained supervisor-memory fault, A's
continued execution and exit, and complete restoration of 28 owned frames,
eight leases, and task/circle/transition state. The
[September 27 execution log](2026-09-27%20Owned%20User%20Execution%20Log.md)
retains the previous 24-target evidence, including its TCG/KVM runs.

The next bounded task is a retained **prepared-launch owner** for the same
three-region GPR-only profile: retain mapping preparation and real task/circle
attachment, distinguish preparation from explicit commit, and support retryable
cancellation of partial preparation or an unstarted ready task. Inject every
preparation failure, require no runnable task escape and exact restoration,
then execute the existing successful user-exit fixture. Input remains inert
and format-neutral; authority is supplied separately.

The [execution-readiness audit](../../docs/architecture/execution-readiness.md)
keeps the later order: kernel launch foundations, native launch contract,
Reliquary execution framing, Linux ELF admission/execution, then an independent
Mach-O/Darwin implementation. General launch, user-copy/fault policy,
scheduler integration for these roots, TLS/FPU ownership, permanent kernel
placement, and global W^X remain open. Environment recovery does not change
those architectural limits.

## Optional evidence from the old machine

Old-machine files were not needed to recover the tracked checkpoint or rebuild
the environment. For continuity, retrieve
an external validation directory (not distributed)
and the complete
an external validation directory (not distributed)
directory, especially `summary.json`, `source-sha256.json`,
`initial-memory-smoke.log`, and the report's linked target/image evidence.
Inspect old uncommitted changes and local branches before importing them;
matching `main` establishes only the synchronized tracked checkpoint.
Desktop captures and other ignored experiment evidence may also be worth
retaining, but generated `build/` and `target/` contents must not be committed.
