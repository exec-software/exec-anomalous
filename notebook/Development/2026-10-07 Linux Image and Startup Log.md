---
title: "Linux Image and Startup Log"
date: 2026-10-07
tags: [linux, elf, userspace, startup, antimatter, memory]
status: qualified-host-only
domain: execution
---

# Linux Image and Startup Log

## Intent and implemented boundary

The user confirmed work toward running the existing Linux Antimatter application
through Anomalous, following ADR 0073. The native reference remains a regression
baseline. This slice implements ADR 0074's userspace-compatible, allocation-free
ELF64 image planning and failure-atomic Linux initial-stack construction. It
does not execute the dynamic linker or any ordinary Linux application inside
Anomalous, and it does not change kernel admission or the old ANEL contract.

The opt-in portable modules accept caller budgets and borrowed immutable source
bytes. They describe multi-segment ET_EXEC/ET_DYN layouts, exact copy/zero ranges,
entry/PHDR, interpreter, dynamic, TLS and RELRO metadata, and a caller-owned
initial-stack template. The host composition tool validates main/interpreter/
stack separation, selects the interpreter entry and keeps the main entry in
AT_ENTRY. Source hashes bind the held bytes. Credentials, CPU capabilities and
entropy quality remain caller responsibilities rather than invented defaults.

The host inspector explicitly supplies the interpreter. It records the image's
declared interpreter byte path but does not authenticate the supplied file or
resolve a guest namespace. Its empty environment and core-only auxv are an
inspection profile. No artifact is converted, patched, recompiled or loaded into
the guest; Antimatter source and installed application files are unchanged.

## Exact artifacts inspected

The independent Python oracle decodes ELF headers itself, compares every raw
program header and LOAD extent, materializes each copy/zero operation, and
decodes the generated stack without calling the Rust parser. Conventional
`readelf -W -h -l` output is retained too.

| Artifact | Bytes | LOADs | 4 KiB pages | SHA-256 |
| --- | ---: | ---: | ---: | --- |
| `/usr/bin/true` | 35,288 | 4 | 9 | `913a39cd38f353497086bcf317b12f91f93c23b51869cea763b8340b4f84cfd3` |
| `/lib64/ld-linux-x86-64.so.2` | 254,864 | 4 | 63 | `bda779ea9e9ad234f60315477d7cfd0bdde861cd8ddd9df8cdaf62f18c4cec14` |
| `/lib/x86_64-linux-gnu/libc.so.6` | 2,190,608 | 4 | 545 | `85e64f97e348786a8fb4d9f3d52fec289e2fb86bba20f0731dfe61990525e0f7` |
| Antimatter's Electron executable | 228,130,120 | 4 | 55,998 | `9578f5ece2da6cbba4b5d0fb7e7adaaf8f07d44684bddfdab92024c84d80d2f3` |

Electron is the installed file at
an external validation directory (not distributed).
Antimatter's earlier audit selected version 9.0.0 at source
`23b92422f87ae063c02fa07cb8f4ec5c35922754`, with Electron 44.3.0. The executable's
successful structural inspection says nothing yet about its shared libraries,
Chromium processes, display connection, graphics features or application UI.

The `true` stack uses main bias `0x40000000`, interpreter bias `0x60000000`,
initial RIP `0x60023340`, AT_ENTRY `0x40002db0`, initial RDX zero and a 64 KiB
stack at `0x7fff00000000`. The oracle checks all pointers and terminators, main
PHDR/PHENT/PHNUM, AT_BASE, AT_PAGESZ, copied argv/exec filename, exact supplied
random bytes and 16-byte RSP alignment. Only core auxv facts are emitted.

## Review and focused tests

Twenty new portable test groups cover malformed/partial images, arithmetic,
alignment, permissions, metadata containment, PHDR derivation, entry selection,
path preservation, resource budgets and all-error stack nonmutation. Together
with the nine existing ELF tests, all 29 passed. Three host-tool tests cover
cross-image interval rejection, path/JSON escaping, explicit source budgets and
preservation of existing evidence. Two Python tests cover the independent
oracle's empty-range and bounded-read behavior.

An initial focused run stopped on one Clippy `collapsible_if` warning. The
condition was rewritten and host/freestanding feature linting, xtask linting,
focused tests, xtask build and actual-artifact inspection passed. That initial
failure remains in the evidence; it was not hidden or represented as passing.

Independent read-only review found two oracle issues: empty mapped ranges were
incorrectly treated as overlapping, and Python reads/materialization needed
their own explicit resource caps. Both were corrected before the final source
freeze and regression run. Follow-up review found no further concrete defect.
No reviewer launched builds, VMs or target applications.

## Qualification and memory evidence

The local evidence root is
an external validation directory (not distributed).
`source-freeze.json` binds 452 tracked and nonignored untracked non-Markdown
source paths. `qualification.json` specifies two host oracle jobs, four affected
regression targets and all sixteen AGENTS.md-required targets. All 22 final
qualification jobs passed on those unchanged sources. The implementation
checkpoint is `7288443`, pushed to `origin/main`.

The sixteen required targets are `check`, `smoke`, `fault-smoke`, `circle-smoke`,
`control-smoke`, `rite-smoke`, `scheduler-smoke`, `supervision-smoke`,
`shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`, `input-wait-smoke`,
`shell-smoke`, `semantic-surface-transport-smoke`, `semantic-presentation-smoke`
and `normal-exclusion`. The four affected regressions are
`elf-application-smoke`, `linux-application-smoke`, `shared-veil-smoke` and
`elf-relaunch-smoke`. The two host jobs are the Python oracle regression tests
and the unchanged-artifact/stack oracle. `make check` includes the new optional
ELF tests and host/freestanding linting; the full check completed in 474 seconds.
There is no new guest execution matrix in this host-only slice.

`audit.py --output final-audit-v1` passed after the suite. Its receipt is
`final-audit-v1/receipt.json`, SHA-256
`b7c420781771120b23332ea448ce492866b8a0e13e59aa1ecc9d12df84bd8986`.
It binds all attempted scopes, exact commands, per-job logs, source hashes,
guard code/policy, real artifacts and the generated stack. It verifies that all
recorded guarded jobs were serialized, retains the initial lint failure, and
confirms no unrecorded visible matching unit. All five baseline images remained
byte-identical:

| Image | SHA-256 |
| --- | --- |
| `build/anomalous.img` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` |
| `build/workspace-dev.img` | `8aa7f44f3f3a444e8b5c2124091e39408c4bc1c1c73087719d98c6f236d71408` |
| `build/owned-workspace-test.img` | `7f819e6df5969c4894919215e9fd48f3b6d93743b03505b806ab6eeaa4fc4554` |
| `build/owned-workspace-cpu-test.img` | `cc2b16108fac639bb54014fd241837d1a34d35ccf53f11f85e8fa395ac70e0a4` |
| `build/native-editor-workspace-test.img` | `0d0284832ef85a58c260c341aaa3105c84d68abfa837a6b7923d25e9d5b49acf` |

Every build/test/VM job is serialized under the retained `bounded_run.py` guard:
one Cargo build job and one test thread, MemoryHigh 4 GiB, MemoryMax 6 GiB,
MemorySwapMax 512 MiB, one-second sampling, thirty-second quiet admission,
host available-memory floor 4 GiB and host/scope/all-ancestor PSI monitoring.
Only the owned job group is terminated on a guard failure; unrelated processes
and host policy are unchanged. Initial and successful focused runs are retained
alongside final qualification. Accessible journal and unit information is
corroboration, not complete historical system telemetry.

Across all attempts, peak guarded scope memory was 3,856,605,184 bytes
(3.59 GiB); minimum observed host available memory was 12,412,133,376 bytes
(11.56 GiB). Final qualification alone retained at least 20,948,877,312 bytes
(19.51 GiB) available. All observed scope high/max/OOM/OOM-kill counters remained
zero, the host kernel OOM-kill counter remained zero through the final audit,
and no pressure abort or monitor error occurred. Accessible unit/journal checks
found no adverse owned-job outcome. Missing collected units and unavailable
counters are recorded as unavailable, not historical zeros; absent accessible
systemd-oomd records do not prove complete system-wide coverage. Sampling is
once per second with a retained final sample, not continuous observation.

## Next implementation work

The current eight-region mapping capacity is exhausted by main and interpreter
before adding a stack; libc makes twelve LOADs and 617 pages across the three
artifacts. Fixed native code/data geometry cannot admit these layouts. Preserve
individual owned executable intervals rather than widening one interval over
unmapped or writable holes.

A read-only follow-up audit narrowed the next increment to an opt-in owned
multi-region launch mechanism, preserving fixed-profile constructors. The sample
main/interpreter/stack geometry needs nine user regions and 88 user pages. The
current `resident_plan` also assumes the user tree already includes low
supervisor ancestry; these biases do not. Build the union of required user and
resident table branches instead of inserting a dummy user mapping. The audit's
static calculation is ten owned table pages and four private transition-stack
pages, or 102 frames in eleven leases, excluding borrowed bootstrap/alias
resources. This is a design calculation, not a guest measurement.

The native contract should accept owned ranges, immutable copy spans, exact
initialized executable intervals, stack/RSP and resource budgets. Kernel
`task_bounds`, entry plans, syscall/interrupt return checks and suspended-call
validation must use the same interval set. Retain per-owner mapping counts for
lifecycle accounting. Hostile cases include executable holes and page slack,
cross-region aliases, missing supervisor branches, wrong identities/roots,
unaligned prefix/BSS readback and retryable allocation/population/clear failures.
Kernel mapping receipts validate addressed spans; userspace supplies ELF and
startup interpretation. Existing native payloads can remain unchanged, with
artifact equality verified after implementation rather than assumed.

TLS requires per-task ownership across syscall entry/return, interrupts,
switches and retirement. Existing FS/GS-selector operations assume zero TLS;
implementing ARCH_SET_FS alone would not preserve the required state. libc's
template is 48 initialized bytes and 128 total bytes, aligned to eight. The
unchanged interpreter should perform its own dynamic relocations (including
DT_RELR), library loading and RELRO protection once owned mapping, file and
Linux startup services exist. Current Linux policy still only supports fixed
record-device read/write/poll, yield and exit.

The next guest acceptance remains unchanged distro `true` plus its matching
loader/libc entering through the Linux ABI and exiting correctly. Progress to
ordinary file, process/thread, synchronization, signal and display behavior,
then the actual Antimatter/Electron application. Additional native editor
fixtures do not substitute for that target.
