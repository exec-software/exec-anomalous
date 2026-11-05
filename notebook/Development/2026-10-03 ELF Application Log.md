# ELF application loading comparison

Date: 2026-10-03

## Scope

The next authorized slice after the native editor/service baseline is a bounded
ELF loading comparison. [ADR 0051](../../docs/adr/0051-prove-userspace-elf-loading-against-native.md)
is Proposed. A separately executing unprivileged loader parses a compact ELF,
submits an inert checked plan, and retires before either application route runs.
The same client and service execute through native flat segments and ELF-derived
segments with native INT f4 startup/service semantics. A Linux execution
personality and full Electron/Antimatter remain separate work.

The user also noted Displacer as a possible shared desktop/customization base,
with the existing theme carried over and useful changes contributed back. That
future direction is recorded in the [direction log](2026-10-03%20Native%20Antimatter%20Direction%20Log.md);
no Displacer inspection or implementation belongs to this slice.

## Implementation and admission boundary

- Shared safe, allocation-free ELF parser and canonical 96-byte plan decoder.
- Compact sectionless ELF retains exact inspected RX and initialized RW bytes.
- Separate ring-3 loader performs runtime parsing and truncated-input rejection.
- Role-bound gate rejects failed copies, malformed/changed plans and duplicates;
  a decoded plan supplies no allocation, task or service authority.
- Loader teardown precedes application allocation. Both routes use retained
  50-frame task pairs, initialized-data readback and zero BSS/stack/page slack.
- Feature-only immutable data canary verifies initialized RW reaches all client
  tasks; the default native binary retains its earlier contract.
- Identical normal and queued-packet fault scenarios compare counters, seven
  surface hashes, final pixels, revocation and exact ownership restoration.

## Qualification

Bounded qualification completed on 2026-10-04. All 35 matrix jobs and both
image-restoration jobs passed, and all 302 frozen non-Markdown source files
remain unchanged with no added source files. Evidence is retained under
an external validation directory (not distributed).
Builds/tests run serially with two Cargo jobs, 4 GiB memory high, 6 GiB memory max,
and 512 MiB swap max. The runner samples cgroup/host memory and OOM counts and
aborts its own process group if host available memory falls below 4 GiB or a new
OOM occurs. Earlier native and Ny milestone evidence is preserved separately.

Initial parser and kernel host tests passed. The first target check caught a
Rust macro declaration-order error in the new child module; that was corrected
before qualification. Artifact inspection also caught readonly SHF_MERGE and
redundant SHF_GNU_RETAIN metadata: the ELF-only inspector admits the exact readonly
merge combination, while the unnecessary canary retention attribute was removed.
Default native flags and initialized-data rules remain strict.

Two early VM runs failed the existing `owned_bootstrap_state` guard. Compiled
frames showed roughly 50 KiB of nested bootstrap use, exceeding the required
16 KiB remaining headroom. Out-of-line mapping preparation alone did not fix the
combined footprint. The ELF-only runtime now occupies exclusively retained
supervisor BSS with direct field initialization and a full-session reuse latch.
The guard and stack sizes remain unchanged. No OOM accompanied these failures.

The completed matrix is `matrix-final.json` with per-job logs,
`matrix-final-results.json`, continuous memory samples and
`matrix-final-memory-summary.json`. Restoration has the corresponding
`restore-final` files. `final-receipt.json` verifies the complete job counts,
source freeze, backend artifacts and pixels.

The final matrix used two Cargo jobs, peaked at 2,045,624,320 bytes (1.905 GiB),
and retained at least 21,207,310,336 bytes (19.751 GiB) of host available memory.
Soft-limit, hard-limit, OOM and OOM-kill event counts were all zero, as were the
initial and final global OOM-kill counts. Restoration peaked at 29,442,048 bytes
and also had no memory events. The incomplete foreground attempt had reached
the 4 GiB soft threshold; its partial samples are preserved separately and are
not substituted for the complete final receipt.

The parser suite passed nine test groups; the host harness passed 142 tests;
the ELF-feature kernel suite passed 460 tests and the all-feature suite passed
494. Required repository checks, all extended regression targets, normal-image
exclusion, the actual Linux Antimatter production-module reference trace,
TCG and actual KVM tests all passed. The KVM wrapper records real `accel=kvm`
and `-cpu host` invocations; the guest reports AuthenticAMD versus TCG's
GenuineIntel. Both RAM sizes and each independent capture passed the strict
23-record oracle. Timer counts vary and are not compared.

Each ELF proof executes two loader tasks and eight application tasks across
native-flat and ELF-derived routes. Loader evidence reports 15 calls, one
wrong-role denial, one failed copy, eight invalid/duplicate offers and one
accepted plan. This includes a decodable but unauthorized plan rejected by the
trusted-manifest comparison. Each application route has seven surface states,
six inputs, three normal exits, one contained fault and one queued packet
cleared on revocation. Normal/fault gate counts are 112/12 with 14/0 cooperative
switches. Both routes compare all non-timer counters and hashes; the second
presentation compares every destination pixel byte against the first before
writing. Host captures also match the independent public-pixel oracle.

The compact ELF is 16,520 bytes. Its client has 8,539 RX bytes, 136 initialized
RW bytes, and 7,600 bytes of RW memory; the loader has 19,944 RX bytes and 128
initialized RW bytes. The 96-byte manifest is separate inert metadata.
The default native RX payload remains byte-identical to the preceding proof
(`000374ca2704b22cfa7ce0ba134cbbe5d13617f0438ec0a15fba35c1964488c0`);
full debug ELF hashes can change with source-line metadata.

Retained hashes:

| Artifact | SHA-256 |
| --- | --- |
| Normal image, restored unchanged | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| Workspace image, restored unchanged | `d6c38325c57d33677aca118772f3cf69036293b73bed1bc1bec1e3c275ecd546` |
| ELF proof image, TCG/KVM identical | `42a69f426ace2ede2c388bb37f602e00ca6f4510ef76493327da3a37f789a970` |
| ELF proof kernel, TCG/KVM and reviewed binary identical | `6e585ad78340265870d3d5eafb396200baf5337a3822818eb4fb8500a0a90a09` |
| Compact deployment ELF | `e838262c138d0e18193a8b5968d08da46f21bc6ab83e62c7d9c53882f6b9f369` |
| Enhanced client ELF | `1a0f3dd6d33e5cd60bf0f2b38111776a0b67fd714be457ef1609c60d18de19f3` |
| Loader ELF | `3653b97309f3f4181f6c492b145c9bfd9d989118cffce766aebc1177e83fe7e9` |

`tcg-artifact/` and `kvm-artifact/` retain manifests, full client/loader ELFs,
disassemblies, extracted segments and compact containers. `native-regression/`
and `ny-regression/` retain fresh evidence without overwriting prior milestone
directories. TCG/KVM editor pixel bytes match exactly in their separate PPM
captures; PNG previews are `elf-application-frame.png` and
`elf-application-editor-crop.png`. This qualifies a bounded ELF loader under
native services; it does not qualify Linux startup/syscalls or full Electron.

## Harness recovery and stack evidence

The initial foreground matrix controller was interrupted without a final job
receipt. Its partial log reached the final check command, but it is not counted
as a successful matrix. Partial evidence is retained in
`work/elf-application/interrupted-first-matrix/`. The full matrix was restarted
as the user service `anomalous-elf-matrix-final` with identical memory limits,
continuous samples, job deadlines and process-group cancellation. No source
changed between those attempts.

Read-only emitted-code review is retained in `stack-review.txt`,
`userspace-stack-analysis.json`, `kernel-stack-analysis.json` and
`kernel-stack-review.disassembly`. The loader performs actual runtime header,
program-header and padding checks; it does not return a compiler-folded plan.
Its conservative user-stack estimate is 912 bytes and the client estimate is
896 bytes, both below the admitted 8 KiB. The client emits a real load from the
canary address. The corrected kernel retains 18,112 bytes of runtime BSS;
its memory end is 0x1a3000, below the existing 2 MiB resident ceiling. The known
admitted gate path is conservatively below 5,847 bytes including trap/alignment
allowance, within the 16 KiB transition stack. Fatal formatter paths are outside
that estimate. Existing bootstrap headroom guards remain active.

## Next boundary

Qualify real Linux initial process state and SYSCALL entry, then a small explicit
syscall adapter over shared service operations. A Linux producer paired with the
native service can compare the same editor operations while exercising distinct
execution personalities. ELF parsing alone is never reported as Linux support.

The candidate Linux comparison is an ELF/Linux producer paired with the existing
native service. Extract register-independent channel operations and keep native
INT f4 and Linux SYSCALL adapters separate. Candidate first syscalls are read,
write, poll with timeout zero, sched_yield and exit over explicitly preopened
bounded record-device descriptors. They must not be advertised as Linux pipes:
write queues a record, successful read copies then acknowledges, failed copy
preserves it, and readiness is derived from channel state. Startup must build a
real bounded argc/argv/envp/auxv stack rather than reuse native boot arguments.

Fast entry also requires separate owned EFER/STAR/LSTAR/FMASK state because the
current application CPU session deliberately rejects SCE. SYSCALL does not
select a kernel stack: use the controller-selected supervisor stack before any
push and preserve the user red zone. An IRETQ return can reuse validated retained
frames. The current zero-GS-base invariant favors avoiding SWAPGS in this first
bounded profile. These are audited candidates for the next slice, not completed
interfaces or new compatibility claims.
