---
title: "Owned Loader Sources Log"
date: 2026-10-07
tags: [kernel, userspace, memory, abi, linux, qualification]
status: qualified
domain: execution
---

# Owned loader sources — 2026-10-07

[ADR 0076](../../docs/adr/0076-prove-owned-loader-sources.md) adds the copied-source
ownership boundary following the qualified
[multi-region launch proof](2026-10-07%20Owned%20Multi-region%20Launch%20Log.md).
Implementation checkpoint `a22d7c1` and formatting checkpoint `c2880a9` are pushed.
Final physical and regression qualification passed against the 460-file v4
source freeze.
The product target remains unchanged Linux Antimatter with its Electron frontend,
starting with unchanged `true`, its interpreter and matching libc.

## Source ownership and launch

The default-off `owned_source_test` feature accepts source bytes from an
authenticated current native task into a separately accounted, move-only owner.
The embedding checks the task incarnation, scheduler context, retained root,
privilege stack, circle profile and complete user-copy path. A numeric grant or
copied identity alone supplies no authority. A supervisor scratch buffer is
cleared and verified after each import; it never becomes retained source storage.

Sequential imports advance only after source writes and readback succeed.
Sealing requires every declared byte and revokes append authority. A consumer
borrows the actual sealed owner while preparing destination mappings; source
offsets retain neither an uploader pointer nor a physical alias. Mapping
permissions, initialized executable intervals, stack admission and task commit
remain separate checks. Partial population stays unpublished, and failed source
clears retain cleanup progress and their leases for retry.

The fixture uses two 7048-byte snapshots, each with an explicit 8192-byte/two-page
budget and imports of at most 4096 bytes. These are per-operation resource
budgets, not global file, pathname or image limits. Real Linux image integration
will need appropriately accounted larger sources.

## Physical acceptance

Each producer imports two 1024-byte code prefixes and a 5000-byte unaligned data
pattern through four accepted chunks. Five denials cover incomplete sealing,
wrong grant, replayed offset, a range crossing a mapping boundary and append
after sealing. Producers then overwrite and verify their original data while
the sealed snapshots retain the initial pattern.

The coordinator keeps only four source frames and two leases across complete
producer teardown. Producer tasks, contexts, roots and physical aliases retire
before fresh consumer owners populate their mappings from sealed bytes. This
is retirement of the producer identities and registries, not a promise that
numeric slot/generation pairs are globally unique across newly constructed
registries. Consumers check all 5000 private pattern bytes before dispatch.

Each of the two sequential task-pair lifetimes must complete 64 preemptions,
one attributed UD2 and sixteen survivor callbacks. Final source cleanup must
exercise two failed clears and two retries with exact resource restoration.
The host oracle expects sixteen ordered records, including eight source-proof
records. The first TCG/KVM and VirtualBox runs met these conditions; the final
source-freeze reruns remain pending below.

The timer boundary adds two source-only, one-shot transitions: quiesce all
external lines before producer retirement while preserving the initialized
`ready` state, then restore only IRQ0 after retirement with IF still clear.
Neither transition executes STI or revives failed initialization. Each dispatch
lifetime separately rearms the unchanged PIT cadence immediately before user
entry. The retained v3 emitted inspection and first physical runs cover this
ordering; no user timer-control ABI is introduced.

## Focused checks and memory observations

The retained `focused-v1` attempt passed 566 kernel tests and 311 host-tool tests.
Freestanding Clippy passed for `owned_source_test` and its `workspace_dev`
combination. The `application_profile_test` combination then failed on unused
proof helpers; their feature gating was corrected for `focused-v2`.
The failed attempt remains evidence and is not counted as a successful suite.

All seven `focused-v2` jobs passed: 568 kernel tests, 311 host-tool tests,
freestanding Clippy for the source profile and its workspace/application-profile
combinations, host-tool Clippy, and image construction/inspection. This scope
peaked at 1,035,624,448 bytes with at least 12,004,028,416 host-available bytes.
Sampled scope OOM/kill counters and host OOM-kill deltas were zero; no guard abort
or runner error was reported. The final audit below includes accessible unit/journal evidence.

The earlier `focused-v1` scope peaked at 953,692,160 bytes, with at least
11,934,351,360 host-available bytes. Sampled host and scope OOM counters did not
increase; there was no guard abort or runner error. Builds and VMs remain serialized with one Cargo worker,
4 GiB soft/6 GiB hard/512 MiB swap limits, one-second host/scope/ancestor PSI
sampling and thirty-second quiet admission. These observations cover the
retained scope, not continuous or complete system telemetry.

## Emitted correction and first physical results

The v2 image passed construction but its preboot emitted review rejected the
consumer admission stack: 50,160 bytes used on the 65,536-byte bootstrap stack
left 15,376 bytes, 1,008 below the unchanged 16,384-byte headroom guard. It was
not accepted for physical qualification. Inlining `prepare_owned_sources` into
the still non-inlined consumer helper let the compiler combine temporary
storage. The immutable v3 review measured 48,592 bytes at that checkpoint,
leaving 16,944 bytes: 560 above the same guard. Neither stack size nor guard was
increased. The four user payloads remained byte-identical. This is an admission
checkpoint bound, not a whole-program worst-case stack certificate, and its
narrow margin requires renewed inspection after relevant changes.

The original `physical-tcg-v1` and `physical-kvm-v1` scopes each passed the
strict source oracle at 96 and 320 MiB. Isolated VirtualBox 7.2.6 passed at
320 MiB, followed by the strict transcript verifier; its receipt confirms
power-off cleanup. The screenshot was retained but was not a pixel-semantic
qualification. These runs used the corrected v3 image and remain historical
evidence after the subsequent formatting correction.

The first `qualification` scope stopped at `make check`'s formatting check,
before its later checks or jobs ran. Only `x86_64.rs` module ordering and
`owned_mapping.rs` attribute wrapping were corrected. A new 460-file v4 source
freeze and artifact were retained; `format-v4` Clippy and image jobs passed.
The original source review is preserved, with a separate
`source-format-v4-review.md` appendix binding these two formatting-only changes.
The final v4 emitted review found all 184,320 text bytes and 4096 data bytes
identical to v3. The only loaded differences were two panic-source line numbers
in rodata. The preparation checkpoint and all user payload bytes are unchanged.

## Final qualification

Evidence folder: an external validation directory (not distributed).
The `source-freeze.json` binds 460 non-Markdown paths before all selected final
jobs. `final-evidence.json` selects `format-v4`, `physical-tcg-v2`,
`physical-kvm-v2`, `virtualbox-v2` and `qualification-v3`, retaining every earlier
attempt separately.

The final TCG and KVM scopes each passed at 96/320 MiB using the same image;
VirtualBox 7.2.6 passed at 320 MiB and its separate strict 16-record verifier
passed, with the VM powered off. All five runs establish 160 timer callbacks,
132 dispatches, 128 preemptions, two contained faults, two exits and complete
restoration. Free frames are 24,141 and 81,485 at 96/320 MiB.

`qualification-v2` was stopped by the unchanged guard during `make check`:
host full-memory stalls reached 11.47% over two seconds, above the 10% threshold.
Its host OOM delta and scope OOM/kill/hard-limit counters remained zero. The
attempt is recorded as aborted, not passing. After quiet admission,
`qualification-v3` passed all 25 jobs: all sixteen AGENTS targets, eight affected
multi-region/dispatcher/application/ELF/Linux/Veil/relaunch/native-editor smoke
regressions, and a fresh owned-workspace image. Its `make check` passed every
feature combination, including 612 all-features kernel tests and 311 xtask tests.

The successful full scope peaked at 4,296,892,416 bytes (soft-limit reclaim),
with at least 21,558,525,952 host-available bytes. No guard abort or runner error
occurred in that scope. Final audit and baseline-image closure are recorded below.

| Final artifact | SHA-256 |
| --- | --- |
| `owned-source-test.img` | `ae333b2eacd43fdd965d83a4433476787c7f2b1527915c6009e56d8df3878d59` |
| `kernel.elf` | `8cdeddea8dafcef48cf9583e977bb8a7c3d9ce62d1eb6c5b2d263dfea98a9a0c` |
| `kernel.bin` | `ae8781f0df1cc69d289df43d2ee5a630bfe6555de6ed1d9b2f18aed4de401b38` |
| `kernel.debug` | `4c4e2421d7c67f644de82f53c83bfbfd3c94d3f033bb8b320c595e5769b44955` |

## Evidence closure and baseline images

The final `final-audit-v1/receipt.json` passed, SHA-256
`d1bf72c771ffb7e3910fcc359c2d347167e187cfed64a48fecbbd8c9a004814e`.
It binds all 13 attempted guarded scopes, exact selected commands, serial and
VirtualBox receipts, artifact hashes, 460 unchanged source paths and the
independent source/emitted/baseline reviews. It preserves the failed lint and
format attempts, the preboot stack rejection, and the pressure-aborted suite.
Across retained scopes the peak was 4,297,027,584 bytes and minimum host-available
memory was 11,538,776,064 bytes. Host and scope OOM/kill deltas and hard-limit
event deltas were zero. Accessible journals contain no intervention attributed
to these scopes. Collected unit counters are unavailable rather than historical
zero; one-second samples and accessible journals are not continuous system
telemetry. The 4 GiB soft/6 GiB hard limits and pressure thresholds were unchanged.

Normal and freshly rebuilt workspace images match their saved baselines exactly.
The CPU workspace image also matches, but was retained from before this turn;
`make check` rebuilt/inspected its userspace probe package, not that kernel image.
The fresh owned-workspace image changes one source-line byte
(`prepared_launch.rs` 574 to 579); the native-editor image changes fifteen
source-line bytes across four source files. Independent exact-artifact reviews
reconstruct every changed disk byte as those locations plus the two image
checksums. Executable bytes, entry, section/load geometry, other initialized
bytes, boot data and padding are unchanged. Native-editor ran its full smoke
on the captured image; the final owned-workspace job built and inspected its
image without an additional VM run. No baseline artifact was restored to force
a comparison to pass.

No unchanged Linux executable or Antimatter process has run in this increment.
Actual image/startup integration,
task-owned TLS/CPU state, dynamic mappings/protection and Linux file/descriptor/
startup services still gate `true`. Electron's wider runtime remains subsequent
work. ELF interpretation and Linux policy stay outside the kernel.
