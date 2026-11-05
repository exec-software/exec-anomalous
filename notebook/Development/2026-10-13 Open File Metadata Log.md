# Open-file metadata — 2026-10-13

Continue from `4caaf9a`: the unchanged Linux interpreter reads libc's program
headers, then requests `fstat(fd=3, statbuf=0x7fff0000ef80)`, syscall 5, at return
RIP `0x600294bb`. [ADR 0089](../../docs/adr/0089-publish-open-file-metadata-through-the-linux-veil.md)
records the bounded implementation and explicit virtual-file metadata policy.
Qualification is complete for this bounded increment.

Evidence is retained at
an external validation directory (not distributed).
The runner is byte-identical to the preceding guard, SHA-256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
This task serializes builds, tests, binary analyses and VMs with one Cargo job,
one test thread, 4 GiB soft/6 GiB hard scope limits, 512 MiB maximum scope swap,
the existing host/ancestor pressure thresholds and a 30-second quiet window.
HAG released its previous quiet window before this work began.

## Implementation

The portable metadata callback receives only retained object/version/length
information. It shares the existing publication guard: acknowledgement commits
the sequence without changing the descriptor; rejection preserves state; unknown
outcome or unwinding fences. Four new portable tests exercise retained snapshot
identity, interleaved reads, all handle/owner/sequence failures, publication
failure and an unsigned maximum length without provider I/O.

The Linux adapter explicitly writes 144 bytes in the existing 4 KiB scratch,
including zero padding. Its logical object identity is now separate from the
native source grant. Existing native operations stage those bytes and return
Linux result zero, with copied length 144 in the delayed acknowledgement.
Private scratch is wiped after the callback ends, or by owned fault cleanup.

The first kernel implementation retained three independent journals, admitted
by the actual sealed sequence. A first before-witness is copied through scratch before
changing journal selection. Source reads remain optional for generic copyout.
Independent source review caught an already-staged duplicate admission that
would otherwise trigger a fatal diagnostic; the candidate helper now rejects it.
Previous committed journals, including rejection counters, remain immutable.

Seven lifetimes add metadata stage and unknown-local-result faults to the five
existing cases. Two pre-admission rejection probes exercise an invalid first
destination and premature completion before the metadata journal exists.
The host oracle independently decodes all fields/padding and checks every byte,
saved frame, source tuple and portable offset/watermark. It cannot accept fstat
as a normal terminal stop. Three new oracle tests mutate every metadata byte,
forge matching staged/readback bytes, and confuse result zero with length 144.

The host per-boot deadline is 180 seconds for seven complete source imports and
lifetimes, up from 120 seconds for five. Guest execution and memory budgets are
unchanged. No old emitted addresses or review decisions qualify the new image.

## Focused qualification

`focused-v1` passed all ten jobs: formatting, a 488-file source freeze, 76
protocol tests, 376 host tests, 611 kernel tests, three strict lint targets,
image build and artifact capture. Its source freeze has SHA-256
`a71ce1a2e38d4df4fabeff865cb8e74a7175acfd644561f502e00ac37961ea6e`.

`analysis-v1` captured the fresh kernel inventory and passed the native
structural review. All 3,587 native instructions were checked against the ELF;
53 sized functions contain no indirect transfers, recursive cycles or dynamic
stack adjustment. Maximum native stack use is 3,200/8,192 bytes, leaving 4,992.
`analysis-v2`, `native-review-v1` and `kernel-review-v1` passed the fresh
control/stack and instruction checks. The independent native cross-review has
no open finding. Those reviews bind the first source freeze and image.
Kernel bootstrap use is 37,200/65,536 bytes with 28,336 bytes remaining; the
largest privileged path is 5,424/16,384 bytes including entry allowance. All
recoverable fault and teardown paths are included. Physical observations remain
a separate qualification.

The focused image is
`4fe56cbeb2669c996cc2f30ba9f20a6c826ae11c201b438f222eaccdbba80e59`;
kernel ELF is
`9c0089306120615f15f68431fdf6672d2ec4c2e7a14f9451362c0c8b60c07f0c`;
native ELF is
`eee3ce9df624b87a2d66d4be4bb39e7f8a5d7a94b2bb79a5517de002195547ec`.
QEMU logs copied into the build-only capture predate this increment and are not
physical qualification. The first ordinary guest attempt is recorded below.

HAG requested a new quiet window after the inventory scope. Heavy jobs were
held while agents reviewed code and instructions. Implementation checkpoint
`4122db4` was pushed with its focused tests and pending physical qualification
explicitly recorded. HAG released the window before the guarded review finalizers
and physical test queue resumed.

## First physical attempt and correction

`physical-tcg-v1` failed in its first 96 MiB normal lifetime after successfully
returning all 144 metadata bytes with Linux result zero. The unchanged interpreter
then issued a second `pread64(fd=3, count=840, offset=64)` at sequence 10, with
destination `0x7fff0000eb90` and return RIP `0x6002978e`. The three-journal proof
budget rejected its source read, and the native adapter trapped at its panic
site. All dependent tasks retired, the independent peer survived, and every
reported cleanup/resource check succeeded. This is a failed physical attempt,
not qualification of the seven lifetimes. Its complete log is retained as
`failed-tcg-v1-96M.log` alongside the runner log.

The correction expands retained observation capacity to four, runs stage and
completion rejection probes for each nonempty delivery rather than each read
class, and extends the independent oracle to cover the repeated positional read.
Prior artifacts and reviews remain unchanged. `focused-v2` passed all ten jobs,
including 76 protocol, 377 host and 611 kernel tests. Its new 488-file source freeze is
`10796d88d295c5c98e0fd49532f8551d7baca36186c4f37fe635c9be9342f127`.
`analysis-v3` passed the new inventory/native structure checks: 53 native
functions, 3,553 instructions and maximum stack 3,216/8,192 bytes. `analysis-v4`,
`native-review-v2` and `kernel-review-v2` then passed fresh instruction and kernel control/stack checks. The independent native cross-review
found no issue. Kernel bootstrap and privileged bounds remain 37,200 and 5,424
bytes respectively, including complete recoverable fault/teardown paths.

The corrected image is
`3aad3e5c3b9e7cd18fa3b2c0c21ce8b2d83d0cef722583d6b44b484765582923`,
kernel ELF is
`9272c4e4c883310163fca3d841b44e6bdf3c17fe8c751426efbf932478bb7c4c`,
and native ELF is
`3c7ca440328ea8b33bcf9fd0f61ef6cf6cd179c3124d023f42f1d3a1643fb53c`.

## Corrected physical qualification

`physical-tcg-v2` and `physical-kvm-v2` passed at 96/320 MiB with identical
reviewed image/source bytes. `physical-crosscheck-v2` independently passed all
four transcripts, binding all metadata bytes, complete frames, source extents,
per-delivery probes, state and cleanup accounting.
Each boot emits 183 records over seven fresh lifetimes. The normal unchanged
interpreter receives metadata at sequence 9, repeats `pread64` at sequence 10,
and reaches syscall 9, `mmap`, at sequence 11 and return RIP `0x6002995c`.
The actual arguments are `[0, 2231696, 1, 2050, 3, 0]`: a read-only private file
mapping request for fd 3 at offset zero, with `MAP_DENYWRITE` in its flags.
The request is retained and retired without a Linux return or exit.

| Scenario | Calls / returns | Native calls | Context queries | Final local state |
| --- | --- | --- | --- | --- |
| Normal read/pread/metadata/repeated pread | 11 / 10 | 826 | 11 | offset 832, watermark 10, unfenced |
| Read fault after stage | 7 / 6 | 768 | 5 | offset 0, watermark 6, caller unchanged |
| Read unknown local result | 8 / 7 | 769 | 5 | watermark 6, handles revoked, fenced |
| Pread fault after stage | 8 / 7 | 786 | 7 | offset 832, watermark 7, caller unchanged |
| Pread unknown local result | 9 / 8 | 787 | 7 | watermark 7, handles revoked, fenced |
| Metadata fault after stage | 9 / 8 | 804 | 8 | offset 832, watermark 8, caller unchanged |
| Metadata unknown local result | 10 / 9 | 805 | 8 | watermark 8, handles revoked, fenced |

Every nonempty delivery retains eight rejected stage probes and four rejected
completions. Metadata additionally exercises its two pre-admission rejections,
returns Linux result zero, acknowledges length 144 and makes zero source reads.
All seven lifetimes retire the dependent pair, complete the independent peer
and restore resources. Unknown-local-result cases follow exact valid physical
acknowledgements; they do not claim malformed kernel replies.

## Final qualification

`negative-v1` passed all six missing/short/corrupted executable-source and
library-source refusals against the corrected image. These refusal logs do not
independently establish final physical-resource cleanup. `virtualbox-v1` again
reported `application_cpu_amd_error_pointers` before userspace; the VM was
verified powered off and remains unqualified for the Linux profile.

`qualification-v1` passed all 29 required/affected targets plus final CPU
workspace image restoration. `baseline-v1` verifies all five development images
byte-identical to their starting baselines. The final audit binds all 488 frozen
final source files, fresh v2 reviews and four passing physical transcripts,
while accounting for the failed first attempt separately. `memory-audit-final-v1.json` accounts for every completed runner
scope; `final-audit-v1.json` verifies the complete evidence chain.

Across 18 guarded scopes and 77 attempted jobs, peak memory was
2,343,718,912 bytes (2.18 GiB); minimum sampled host available memory was
19.36 GiB. No OOM, hard-limit event or pressure abort was observed.
The first TCG failure and VirtualBox refusal remain failures in the audit.
The corrected implementation was committed and pushed as `0b39dc9` after fresh
TCG/KVM and independent physical verification. The observed next operation is
the mapping request above; no libc initialization, main `true` execution,
Linux exit or Antimatter launch is claimed.

## Next mapping handoff

The observed request spans 545 pages. Libc occupies 535 pages: its last page
contains 3,344 file bytes followed by 752 zero bytes. The ten whole pages beyond
EOF must not become ordinary readable anonymous zero pages. Linux accepts the
reservation but faults access beyond the file with `SIGBUS`; delivering that
Linux signal is separate from a bounded native containment proof. `MAP_DENYWRITE`
is ignored by Linux. See the [mmap contract](https://man7.org/linux/man-pages/man2/mmap.2.html).
The large initial extent is consistent with
[glibc's segment mapping sequence](https://codebrowser.dev/glibc/glibc/elf/dl-map-segments.h.html);
that explanation is an inference, not evidence of the next syscall after mmap.

A new runtime mapping transaction is needed before answering this request.
`prepared_launch::committed_mapping_mut` forbids rebuilding or changing payloads,
and the current loader caller validation fixes nine mappings and 128 pages.
The bootstrap allocator borrow also encloses userspace execution, so a nested
allocator request from a gate fails. Resolve that ownership handoff or retain an
explicit bounded allocation pool; do not alias the live allocator borrow.
Reuse checked mapping geometry, retained source readers and cleanup patterns,
while keeping virtual extent ownership separate from populated backing pages.
Preserve the whole requested range, immutable source identity beyond descriptor
closure, existing mappings, saved frame and fd offset. Population must stop at
the file length. Rejection cleans unpublished owners; uncertainty after possible
publication fences/retires the dependent tasks instead of rolling back a mapping
that may already have been accepted. Subsequent executable remapping, protection,
unmapping and Linux fault delivery retain their own qualification gates.
