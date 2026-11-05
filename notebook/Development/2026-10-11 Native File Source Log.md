# Native file source log — 2026-10-11

## Scope

Continue from `ae3c404fda42722110f3787cff13ac98fa97c2aa`, the portable
read-only file service checkpoint. ADR 0085 adds a separate retained libc source
and native bounded reads while the unchanged interpreter's fourth request stays
suspended. This checkpoint does not add Linux open/read replies or advance true
or Antimatter startup.

The configured libc snapshot contains 2,190,608 bytes with SHA-256
`85e64f97e348786a8fb4d9f3d52fec289e2fb86bba20f0731dfe61990525e0f7`.
Its 535 owned pages and one additional bound lease are separate from the
unchanged executable/interpreter/startup source and 88 Linux user pages.
Native userspace consumes 17 bytes, then 4096-byte chunks and the final tail:
536 successful reads. Ten rejected reads preserve the BSS buffer, initialized
canary, RX entry byte and end-of-RW witness. All source-read calls use pending
sequence 4.

Host SHA-256 pins the input. FNV64 compares retained source, delivered stream and
native-consumed stream; it is integrity bookkeeping, not authentication. The
new source binding and diagnostic do not carry Linux namespace policy.

## Evidence and memory policy

Task-local evidence is retained at
an external validation directory (not distributed).
Earlier evidence directories remain immutable. The copied memory guard retains
SHA-256 `19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`:
one build/test worker, one VM, MemoryHigh 4 GiB, MemoryMax 6 GiB, swap 512 MiB,
30 seconds of quiet pressure before jobs, 4 GiB available-memory floor and the
existing per-host/ancestor/scope PSI thresholds. The launcher changes only the
transient unit prefix to `anomalous-linux-file-source-`.

Qualification passed. Fresh emitted kernel/native stack and control reviews
closed before boots, and an independent final audit matched the frozen source,
artifacts, physical records and successful receipts. Source review or the
userspace report alone does not establish a physical run. Every attempted
failure remains separately retained.

## Reviewed build and preserved attempts

The first focused build passed the protocol/host tests and lint, then the native
ELF inspector rejected SHF_GNU_RETAIN from three redundant `#[used]` attributes.
Removing those attributes preserved linker KEEP retention and the existing
strict section checks. The second build passed native inspection but found a
local `retained` binding shadowing the runtime tuple struct. Renaming that local
to `retained_state` fixed compilation. Both failed attempts remain in evidence.

The third image build passed. The final non-Markdown source inventory contains
485 files; `source-freeze-v3.json` SHA-256 is
`7488158fa0b5d94816679ad29a333323e1768a9e2fa8e23b541f0a0b7e9a65d5`.
Earlier focused checks began before the first inventory was captured and are
supplemental only; final acceptance requires the later complete `make check`.
The affected kernel profile separately passed 615 tests and strict lint.

Fresh emitted reviews bind this source and the retained image. The bootstrap
mechanism uses 46,480 of 65,536 bytes, leaving 2,672 bytes beyond its required
16 KiB reserve. Syscall, native gate, timer and fault-prefix bounds are
5,408 / 5,216 / 3,296 / 2,448 bytes on 16 KiB stacks. These kernel mechanism
bounds exclude generic formatter/fatal recursion and NMI nesting; the fault
bound is a fail-stop prefix, not recovery. The native veil's complete direct
control graph, including failure paths, uses at most 1,080 of 8,192 bytes.
Its source-read path uses at most 856 bytes from entry. Separate mapping evidence
binds the full two-page native RW/NX mapping and the end-of-mapping witness.

`check-v1` was stopped by the unchanged memory guard: global full PSI over a
rolling two-second interval reached 11.467761%, exceeding 10%. The job received
SIGTERM; unit status records exit-code failure, not a kill. Peak scope usage was
4,297,060,352 bytes, with 436 memory.high events and no memory.max or OOM events.
The global signal does not establish which process caused the pressure.
`check-v2` passed the complete check under the same limits in 731.42 seconds,
including all 361 xtask tests. Its peak was 4,297,023,488 bytes with 6,818
memory.high events, no memory.max or OOM events and no pressure abort. Host
available memory stayed above 19,102,707,712 bytes during that scope.

## Physical source reads

Fresh TCG and KVM runs passed at both 96 and 320 MiB. Each run recorded four
Linux invocations, three returns and 666 native calls. Native userspace consumed
the full source in 536 successful reads; all ten rejected probes preserved
their witnesses. Retained-source, delivered and guest-consumed FNV64 values
were all `18142536360841512133`. The fourth invocation's request and full saved
frame stayed unchanged. Final source/scratch/request/TLS clearing, CPU restoration
and owned-resource restoration passed.

The physical runs retain separate serial streams and immutable artifact bundles.
The reviewed build's image SHA-256 is
`397f110b04c4c4719db51e6721f47f507b05ecf862861c3049bbc1da10e378c2`;
the existing launch source, plan and ANLPOL03 policy are byte-identical to the
preceding checkpoint. Independent carry-forward reviews confirmed that all 21
non-serial artifacts from both backends exactly match the reviewed build. KVM
invocation evidence records the real KVM accelerator, host CPU and both RAM
sizes without a fallback.

All six negative provisioning boots passed: missing, truncated and one-byte
corrupted launch and file sources each refused before userspace entry with the
expected missing/size/hash reason. These fatal-path logs do not expose an
independent physical-resource cleanup measurement.

The VirtualBox 320 MiB attempt used the same image and again refused with
`application_cpu_amd_error_pointers`. It did not reach the source operation and
is not a semantic pass. The harness retained the failure and confirmed the
private VM powered off.

All 28 remaining regression commands passed in `qualification-v1`. Together
with `check-v2`, this covers all 16 repository-required commands and 13 affected
commands exactly once on the final source. The regression scope peaked at
2,176,831,488 bytes; host available memory stayed above 18,067,308,544 bytes.
It recorded no memory.high/max/OOM events or pressure abort.

Four baseline images are byte-identical. The native editor workspace image
changes from `7ad99460d9ed6aaaf2d34c9042f32dab134e0ae3cf275822c01016cabcd962ed`
to `bf807d8af33ce250752baa060af510c6849cbb05cdd5a6250e688918502b47a6`.
An independent binary review accounts for every changed byte: three Rust
`Location` line fields in `.rodata` increment from 814/777/2331 to 815/778/2332
in `owned_execution.rs`, plus eight Threshold checksum bytes. The added cfg
line shifts those unchanged indexing statements' diagnostic locations.
The matching profile's 98,304-byte `.text` is identical. Its executable LOAD
also contains `.rodata`, so the whole segment is not byte-identical.

`baseline-detail-v1` rebuilt and captured the matching native-editor ELF, raw
kernel, debug file and image to resolve this difference; the image exactly
matches the one that passed the regression. It then restored the owned CPU
profile and verified all five images and the kernel ELF/bin/debug against the
retained final baseline capture. This scope peaked at 164,483,072 bytes with
no pressure abort or OOM events.

## Final evidence audit

`final-audit-v1/receipt.json` passed and binds `final-evidence-v1.json` to the
unchanged 485-file source inventory. It retains all 13 attempted scopes and 54
jobs, selecting 44 successful final jobs and six earlier supplemental checks.
The two failed focused builds, pressure-aborted check and unsupported VirtualBox
attempt remain visible. Each of the four final physical serial streams contains
exactly 19 records; the independent verifier checks source bytes, complete saved
frames, actual live pathname, guest report and final restoration.

Across all guarded attempts, peak scope usage was 4,297,060,352 bytes and minimum
host available memory was 18,067,308,544 bytes. Observed host and scope OOM deltas
were zero, with no observed OOM kill. The audit reconciles retained unit and
accessible journal evidence as well as sampled counters. Journals are partial,
and one-second PSI sampling is not continuous system-wide observation.

Key immutable receipts in the evidence directory:

| Receipt | SHA-256 |
| --- | --- |
| `final-evidence-v1.json` | `d336ca5fcd366611113abf7480cd3549b8ebb7193322146eb09139392695de00` |
| `final-audit-v1/receipt.json` | `af32715922b8df0ecbade32d6de2e2b9cefd3a2d4705aadf7d6f462be4132c71` |
| `emitted-v1-review.json` | `8293e0ede423e3b522a2876dc5b3e82bdc353d9dbebec0d41cc6999d7c27aad9` |
| `native-emitted-review-v1.json` | `ebc5aa79db3914e9c5d8d26aa2135fb62d17c8eceed15ba4e20a479fe23f7a33` |
| `native-rw-map-supplement-v1.json` | `5d76700eaf28e0ac12a59ffaa06afa40f96358e550c4a8f6d0c8fa44f3485805` |
| `baseline-native-editor-reviewed-v1.json` | `2c4024fee73286b4aa7f0023bc9c0f68d3be5d9f3fe6125d27d74759377a97c0` |

The manifest also binds the two physical artifact carry-forward reviews, six
negative cases, VirtualBox classification, restored baseline profile and exact
audit/guard scripts. Final documentation is reviewed separately after this
audit to avoid a circular document/receipt hash dependency.

## Remaining integration

The portable file service still borrows contiguous bytes. Adapt it to bounded
source reads without fabricating a slice across the physical alias. Supply
trusted caller/service incarnations and connect staged copyout to exact reply
completion. Commit local descriptor/offset effects only after the delayed
completion acknowledgement, before handling the next invocation. Unknown
completion or service faults require fencing and dependent-caller containment.
The guest namespace still declares only preload/cache absence; no answer for the
observed hwcaps libc path is installed by this source proof.
