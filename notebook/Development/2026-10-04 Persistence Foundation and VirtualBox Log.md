# Persistence foundation and VirtualBox — 2026-10-04

## Scope

Continue after ELF replacement checkpoint `79c3b41`. The user selected
VirtualBox for interactive testing while retaining QEMU for development, and
asked that OOM prevention and frequent commits/pushes continue.

The storage audit found no runtime block-device path. Threshold's BIOS reads
are confined to boot. Proposed [ADR 0060](../../docs/adr/0060-model-bounded-durable-document-recovery.md)
therefore starts the persistence slice with executable userspace recovery
policy over an explicit sector adapter. The current native/ELF clients remain
RAM-only. Native device access and a real cold-VM persistence proof are still
required before claiming guest filesystem durability.

The new safe `no_std` module grants one writer an exclusive five-sector
adapter. Formatting requires an all-zero disposable extent; recovery never
formats. Two canonical body/commit pairs retain exact bytes, content version,
mount epoch and sequence. A save publishes its candidate RAM state only after
body/flush/commit/flush. Unknown I/O quarantines the store. Mount commits a new
epoch before exposing handles, so old wire identities cannot survive a service
restart. CRC32C detects accidental corruption under the documented model; it
is neither authentication nor protection against rollback.

## VirtualBox host findings

VirtualBox `7.2.6_Ubuntur172322` is installed on kernel `7.0.0-34-generic`.
The earlier inference that root-only `/dev/vboxdrv` prevented startup was
incorrect. The installed udev rule deliberately makes that node root-only,
`/dev/vboxdrvu` is accessible, and the packaged launchers mediate driver access.
Actual startup succeeded without modifying permissions or using sudo. Old
frozen evidence remains unchanged; these actual probes supersede that readiness
inference. No QEMU VM was running during the VirtualBox probes.

`scripts/virtualbox-proof.py` creates a private `VBOX_USER_HOME`, unique
`VBOX_IPC_SOCKETID`, UUID, copied
boot disk and VM config for each new output directory. It records command
results and process cgroups, uses bounded logs/waits, captures a PNG, and
confirms poweroff in cleanup. Existing user registrations and disks are never
adopted. A matching event triggers capture only; the receipt does not call that
semantic proof acceptance. Thirteen mocked tests cover ownership, bounds,
namespace isolation, startup and termination cleanup, final transcript
truncation and error propagation.

Actual 512-MiB, one-CPU runs:

- Normal image reached `boot.ready`, the interrupt-backed shell and its prompt;
  the captured 1024×768 shell rendered correctly on visual inspection.
- Workspace image reached `workspace_service.ready` and frame sequence 1 with
  768 rows; its 1024×768 desktop capture rendered correctly on visual inspection.
  All 786,432 normalized RGB pixels also match the retained QEMU initial
  workspace frame exactly (`workspace-capture-review.json`).
- Native and ELF replacement images stopped at the preserved CPU admission
  guard with `application_cpu_amd_error_pointers`, before application execution.
  These are failed application proofs, not passes or graphics failures.

The guest advertises CPUID `0x80000008.EBX=0x24001000`, while the host reports
`0x791ef257`. Required bit 2 (`XSaveErPtr`) is absent in the guest. Official
[VirtualBox 7.2.6 source](https://github.com/VirtualBox/virtualbox/blob/v7.2.6/src/VBox/VMM/VMMR3/target-x86/CPUMR3CpuId-x86.cpp#L2374)
explicitly masks it out. Overrides/profile values are loaded before this
sanitizer, so selecting a host profile does not restore it. Upstream
[033c7fa9](https://github.com/VirtualBox/virtualbox/commit/033c7fa9bdfaf6b586642a1ca3d3c86c4a8154a9)
added passthrough; later
[c51dc79b](https://github.com/VirtualBox/virtualbox/commit/c51dc79b00f36b2fe289bc64c4d6dae30dd885ff)
commented it out while expanding other flags. A regression is plausible, but
the removal's intent was not established. No CPU spoof, forced capability or
kernel admission change was made. An upstream correction still needs exact
floating-point save/restore qualification before native/ELF acceptance here.

The runner's fresh service and VM processes were observed in the bounded
systemd cgroup. All tested VMs were confirmed powered off. The existing
`Anomalous Dev [Threshold]` registration was not used for these isolated probes.

The first attempt to refresh that existing development VM exposed a separate
tooling issue. After a private-home probe, a default-registry command in the
same systemd unit reused the still-running private VBoxSVC. It could not find
the development VM and attempted creation; the existing config-file guard
stopped it without replacing that VM or its disk. A separate unit then refreshed
the existing VM successfully with `workspace-dev.vdi`, 512 MiB and two CPUs,
and left it powered off. Read-only observations subsequently saw this interactive
VM running and then powered off at 23:36:51 UTC; no further start/stop command
was issued.
The previous generated normal VDI remains available.

The proof runner now sets a fresh socket identity as well as a settings home,
matching [VirtualBox's own validation harness](https://github.com/VirtualBox/virtualbox/blob/v7.2.6/src/VBox/ValidationKit/testdriver/vbox.py#L1054).
The socket path is 66 ASCII bytes, within Linux's limit. Final tests kept the
ordinary service active with repeated registry reads while each of four
sequential proof VMs ran. Every default-registry sample remained unchanged;
the private service and VM remained inside the bounded cgroup. Normal/workspace
boot and rendering succeeded again, with the full workspace frame still equal
to QEMU. Both replacement images produced the same safe CPU rejection. The
`final-vbox-*/coexistence-verified.json` receipts distinguish these outcomes.

## Completed validation

Local receipts are under
an external validation directory (not distributed).
Builds use two Cargo jobs and one test thread. Heavy commands and VMs are
serialized under `MemoryHigh=4G`, `MemoryMax=6G`, `MemorySwapMax=512M`; monitoring
aborts on a new host/scope OOM or less than 4 GiB available host memory.

The first focused compile found a test-only Rust tuple-struct shadowing error;
renaming the test observer fixed it. The first Clippy run found a test-only
explicit drop of a non-Drop value; a lexical scope now ends that borrow. Both
attempt outcomes are retained in the receipts. The final retry replaced the
shared Clippy log; `preflight-clippy-diagnostic.txt` records the initial
diagnostic copied from tool output. Production recovery semantics were unchanged
by these corrections.

Implementation checkpoint `0db6240` is pushed. All 21 sequential qualification
jobs passed: the 16 commands required by `AGENTS.md`, additional native/ELF
replacement and workspace smoke tests, and normal/workspace image restoration.
The run took 1,088.81 seconds. Its 373 non-Markdown source files remained frozen.

The later IPC fix is pushed as `67d259e`; it changes only the two Python harness
files. All Rust, kernel, loader, build and other script sources remain identical
to the full-suite freeze. That final harness separately passed 13 mocked tests
and all four live coexistence checks. A second 373-file source freeze records
the final implementation; the full kernel matrix was not repeated for this
Python-only correction. A separate freestanding document check and final
development-VM configuration/readback also passed.

Focused results: 28 document unit tests, ten exhaustive crash-model tests,
221 xtask tests, warning-denied document Clippy and both saved-frame verifier
commands passed. The native and ELF QEMU replacement proofs passed at 96 MiB
and 320 MiB with separate captures. Workspace passed 21 keyboard frames,
410 pointer/Appearance frames and 23 graphics frames, plus fault/watchdog,
malformed-context/mailbox, resource restoration and shell fallback checks.
The new durable module remains unconnected to a guest device.

All four final raw images are byte-identical to the preceding qualified images:

| Image | SHA-256 |
| --- | --- |
| Normal | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| Workspace | `bd4a51a6e7a8ab9b6d9e4c1ce4df2112d2c21c85e70eeaa66b26b5e8419266ca` |
| Native replacement | `3888955219aa61d9ba1701276c4759bc8a5783f350222b3dd2aae171c27b9bb5` |
| ELF replacement | `5bec05b6521ab064e13826049dd2c8cd15bdf0aa35bf3cb4b2fc4f761bc2bf40` |

Across all recorded phases, peak scope memory was 3,056,222,208 bytes
(2.85 GiB), and minimum host available memory was 15,198,937,088 bytes
(14.16 GiB). Host and scope OOM counters, hard-limit events and soft-limit
events all remained zero. Exact results, images/captures, process cgroups,
source snapshots and both failed preflight outcomes are retained locally.
The final 1,640-file `SHA256SUMS` manifest has SHA-256
`2dfea6a16a977a6631f95d582cadb836c3e9339f3441647ef8b89a9ec629d47c`;
`sha256sum --check --quiet SHA256SUMS` passed.

When the workspace VM is powered off, open it without rebuilding using:

```sh
VBoxManage startvm 'Anomalous Dev [Threshold]' --type gui
```

For later source changes, `CARGO_BUILD_JOBS=2 make -j1 workspace-virtualbox`
rebuilds and refreshes it. The VM must be powered off before that refresh.

## Next native integration

Define a separately granted disposable data device and bounded read/write/flush
mechanism. Keep on-disk policy in userspace. Authorize the current task and
extent before any I/O, preflight complete copies, and quarantine uncertain
commands. Current IF-clear gates cannot use PIT ticks as a polling deadline.
The later proof must destroy both service and editor, then cold-boot a fresh VM
against the same retained data medium. VirtualBox's correct data LUN must have
`IgnoreFlush=0`; QEMU must preserve flush semantics and report I/O errors.
This remains separate from physical host-power-loss guarantees and a general
filesystem or full Antimatter/Electron port.
