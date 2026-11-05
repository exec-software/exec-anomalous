# Positional file reads — 2026-10-13

Continue from `2929b37`: the existing Linux interpreter opens libc and reads
832 bytes, then requests `pread64(fd=3, count=840, offset=64)`. The next slice
implements this request through the existing native source/stage/reply path.
[ADR 0088](../../docs/adr/0088-read-linux-files-without-advancing-descriptor-offsets.md)
records the accepted bounded scope. Qualification is complete for this increment.

Evidence is retained at
an external validation directory (not distributed).
The runner is byte-identical to the preceding guard: SHA-256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
Builds, tests, binary analysis and VMs run serially with one Cargo job and one
test thread, 4 GiB soft/6 GiB hard scope limits, 512 MiB maximum scope swap,
the existing host/ancestor PSI thresholds and a 30-second quiet window.

## Implementation

The portable service now exposes positional reads through both byte-slice and
bounded-source APIs. Explicit source offset and committed descriptor offset are
separate values in one shared read plan. Success advances only the request
watermark; known rejection leaves state intact, and uncertain publication fences
the service. Host tests exercise interleaved sequential/positional reads, wide
offsets, EOF/zero requests, source errors, identity/sequence/handle rejection,
publication failure and unwinding.

The native adapter admits nonnegative signed Linux offsets and uses the existing
authenticated source and staged publication gates. The kernel retains two
independent BSS delivery journals; each source extent is admitted before I/O and
each caller baseline is captured before its rejection probes. Earlier journals
remain immutable after the next one is selected.

Five fresh lifetimes preserve the existing read fault cases and add faults after
positional staging and after accepted positional delivery with an injected unknown
local outcome. The independent host oracle checks both actual byte ranges and
descriptor state, and cannot qualify a supported read as the terminal operation.

## Retained attempts

`focused-v1` stopped at a new test callback's lifetime inference error. An
explicit `publication<'_>` callback parameter fixed the shared slice/scratch
test and native callback. `focused-v2` then passed all ten jobs: 72 protocol
tests, 373 host tests, 611 kernel tests, strict lint and the image build.

`analysis-v1` generated the kernel inventory but rejected the native artifact:
the compiler lowered the four fault-mode match arms into an indirect jump
table. The native proof previously required direct control flow. An explicit
validated parity branch preserves that constraint without relaxing the checker.
No guest ran from either rejected attempt. The rebuild and fresh review remain
separate retained evidence.

`focused-v3` passed formatting, a fresh 488-file source freeze, native/kernel
lint, image build and capture. Only the native fault selector changed since the
complete focused tests. `analysis-v2`, `native-review-v1`, `analysis-v3` and
`kernel-review-v1` passed fresh structural, instruction, control and stack
reviews. A separate native cross-review found no issue. Native stack use is
3,192/8,192 bytes; kernel bootstrap use is 37,200/65,536 bytes with 28,336 bytes
remaining, and the largest privileged path is 5,424/16,384 bytes including entry
allowance. Full recoverable fault and teardown paths are included.

`physical-tcg-v1` passed at 96/320 MiB with identical reviewed image/source
bytes. Both ordinary boots emit 123 records over five fresh lifetimes. The normal
run opens fd 3, reads 832 bytes, receives 840 bytes from source offset 64 through
`pread64`, and reaches the actual next syscall 5, `fstat(fd=3)`. There are nine
calls and eight Linux returns; sequential offset stays 832 and the service
watermark advances to 8. The final unsupported request is retired without a
Linux return or exit.

| Scenario | Calls / returns | Native calls | Context queries | Final local state |
| --- | --- | --- | --- | --- |
| Normal read and pread | 9 / 8 | 790 | 8 | offset 832, watermark 8, unfenced |
| Read fault after stage | 7 / 6 | 768 | 5 | offset 0, watermark 6, caller unchanged |
| Read unknown local result | 8 / 7 | 769 | 5 | watermark 6, handles revoked, fenced |
| Pread fault after stage | 8 / 7 | 786 | 7 | offset 832, watermark 7, pread caller bytes unchanged |
| Pread unknown local result | 9 / 8 | 787 | 7 | watermark 7, handles revoked, fenced |

The unknown-result cases inject uncertainty locally after an exact valid
physical acknowledgement. They do not claim a malformed kernel reply. Each
nonempty delivery has eight rejected stage probes and four rejected completion
probes; source offsets and actual caller bytes are checked independently against
the retained libc. The independent peer completes and resources are restored in
all five lifetimes.

`physical-kvm-v1` reached four completed lifetimes (98 retained records), then
hit the old 60-second per-boot harness deadline during the fifth lifetime. The
partial output remains a failed attempt, not a five-lifetime qualification.
The test previously ran three lifetimes; its host deadline is now 120 seconds
for five complete source imports and service runs. The memory guard and guest
execution budgets are unchanged.

`focused-v4` rechecked all 373 host tests, formatting, host lint and the image
build. The 488-file final source freeze differs only in the host deadline and
its comment. `deadline-review-v1.receipt.json` binds that exact source change
and all 14 unchanged guest artifacts to the existing emitted reviews.

`physical-tcg-v2` and `physical-kvm-v2` then passed at both 96/320 MiB under the
final harness, with all five lifetimes and 123 records per boot. Their captured
guest artifacts match the reviewed bytes exactly. The independent physical
crosscheck passed all four transcripts, including exact source ranges, caller
bytes and full saved frames. A separate KVM review found no issue: scenario
records match TCG apart from the reported CPU vendor. The libc program-header
range `[64, 904)` has SHA-256
`74edaf260a7f17bbc93c8afe7f840fda2aff75c9807a65ef22c570f8766a7c58`.
The actual next request is `fstat(fd=3, statbuf=0x7fff0000ef80)`, syscall 5,
at return RIP `0x600294bb`. It remains suspended without a Linux return.

Implementation checkpoint `3f60ac8` was committed and pushed after the focused
checks, all four final QEMU boots and independent physical crosschecking.

Final image SHA-256:
`189b9f543c15a2ea84c37c29379841e76987a9a8a929ea70eac965f46e4b47cf`.
Kernel ELF SHA-256:
`ac4c60334b8fa1e05e39892521657be3649b2ac2511f985e2ff5810f60d22b46`.
Native ELF SHA-256:
`85e775d4b7e9bf3a274ac3945b4e5546513c3df62ead4425e99b75df9d6c3367`.
The final 488-file non-Markdown freeze, `source-freeze-v4.json`, has SHA-256
`f9c0a2af726b385ca0c37e9ffad8779aecce9e85bf7ac8e6663276555f2a7f10`.
The held review bundle is `focused-v3-artifact`; the host-only deadline change
produces identical guest artifacts in `focused-v4-artifact` and both final
physical bundles. Preexisting QEMU logs copied into build-only captures are
not physical qualification; only the final `tcg-v2-artifact` and
`kvm-v2-artifact` logs establish those four boots.

All six source-provisioning refusal cases passed for missing, truncated and
single-byte-corrupted main/libc sources. These establish refusal before a user
boundary, not independent physical cleanup accounting for provisioning failure.
VirtualBox 7.2.6 at 320 MiB refused `application_cpu_amd_error_pointers` before
userspace and was powered off. Its receipt retains the nonzero result and
`semantic_qualified: false`; source provisioning on VirtualBox remains open.

## Final qualification

All 29 required/affected commands and the final CPU workspace image restoration
passed. The final source check retains all 488 exact non-Markdown files;
documentation changes do not alter the tested implementation. All five baseline
images match their starting bytes exactly, with no metadata exceptions needed.
ADR 0088 is accepted only for this bounded integration.

Across 19 retained guarded scopes and 81 attempted jobs, observed peak scope
memory was 2,786,566,144 bytes (2.595 GiB), with at least 20,763,914,240 bytes
(19.34 GiB) available on the host. No soft/hard-limit event, OOM kill or pressure
abort was observed. This task's builds, tests, VMs and binary analyses ran
serially with one Cargo job and one test thread. Complete runner endings,
sampled host/cgroup counters and accessible unit journals support these claims;
collected units have no surviving historical exit status. The compile failure,
indirect-transfer rejection, first KVM deadline failure and VirtualBox capability
refusal remain failed attempts in the audit.

Final retained receipts:

- `final-audit-v1.json`: `5d046ff262ebda727d3bb132d33b4056c1c16159b100bb6f59c9dda210ec4402`.
- `memory-audit-final-v1.json`: `1ffda4786e9ca1d6ad86725ee56ad2f1937a85dd53ceb35addc35246e8c2298d`.
- `baseline-review-v1.json`: `11bd9ae35f9eaaba623f4b1bcda321e6f6c12f928c2573d036afe0b0fdb181a7`.
- `deadline-review-v1.receipt.json`: `ec7843ab92e6ae5ccca1e9047f0466e3d660fb403fc00c5235c2816c9f5cd514`.
- `physical-crosscheck-v1.json`: `c9581e4f79ead3723778dbd9f74231d8a90760a24a994b442cf1225d0865be0d`.
- `kernel-emitted-review-v1.json`: `e8c613aa6eeade0360048c901b71589932211954b774ceec3d637f88ca09485c`.
- `kernel-emitted-semantics-v1.json`: `0d7a5698f0bae87715c460a4cda0493137d0f8c772fd73311b75457451b6f4a4`.
- `native-semantics-v1.json`: `64d550e791f136d91c28f7c116ad7067b034b8b381e2694e023ad6035ef61b2b`.
- `native-crossreview-v1.json`: `dacb9e7089b37c5442ca7576de6bc935ff98b734c061720ca28465706d315b81`.

Neither libc mapping/startup, completion of `true` nor Antimatter launch is claimed.

## Next observed gate

Implement transactional metadata publication for the retained open file. Its
snapshot identity and sequential offset must survive success or known rejection;
only confirmed publication advances the service watermark. Uncertain publication
must retain the existing fencing contract. Encode Linux metadata explicitly in
userspace with a documented metadata policy rather than importing host authority.

The current copyout proof couples journal selection to a source read and expects
source bytes to equal staged bytes. Metadata needs a delivery with no source
read while preserving the two earlier observations. Generalize that bounded
copyout bookkeeping without adding Linux metadata interpretation to the kernel.
Existing staged completion already separates the Linux result from the copied
length. The next proof must independently decode actual caller metadata, retain
the fault cases, and observe the real following unsupported syscall.
