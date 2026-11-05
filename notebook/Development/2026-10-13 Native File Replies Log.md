# Native file replies — 2026-10-13

## Scope and starting point

Continue the unchanged Linux Antimatter direction by joining the bounded file
backend to real Linux loader requests. Start from clean, pushed main
`76a18ee9152e21e6c9d8be460aebc70acfb96d1d`. The previous guest proof supplied three
startup replies and read libc into the native veil, then retired at the fourth
request. It did not return a successful Linux file open or read.

[ADR 0087](../../docs/adr/0087-return-native-file-service-replies-to-the-linux-loader.md)
records the accepted bounded integration and containment contract. Final
qualification is complete; the earlier failed attempts remain separate below.

## Evidence and memory policy

Fresh evidence is retained outside the repository at
an external validation directory (not distributed).
Earlier completed evidence directories are unchanged. The initial host sample
reported zero OOM kills and zero memory PSI averages. The retained guard is
byte-identical to the previous checkpoint, SHA-256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.

Builds and VM jobs run serially with one Cargo job and one test thread, under
MemoryHigh 4 GiB, MemoryMax 6 GiB and MemorySwapMax 512 MiB. The guard requires
30 quiet seconds, retains a 4 GiB host-available floor, observes host and all
cgroup ancestor pressure domains, and stops at the existing pressure thresholds.
Failed attempts remain separate evidence. Fresh emitted review must precede
the first guest boot of the changed kernel and native service.

## Implementation observations

The synchronous portable publication callback can remain suspended inside the
native reply operation until the unchanged caller issues its next syscall.
This preserves the existing service transaction model. The delayed exact
acknowledgement precedes local descriptor/offset commit and the next request's
dispatch. An uncertain local result fences the service and retires the dependent
caller without replaying the already accepted reply.

The expanded transcript and copyout witnesses belong in kernel BSS. Returning
them by value would consume the limited remaining bootstrap stack reserve.
Native userspace reuses its existing 4 KiB source buffer; physical erasure after
abort belongs to the manager because Rust destructors do not run on a native
fault. Normal native returns add explicit volatile erasure outside the borrowed
publication slice's lifetime.

## Qualification

The initial `focused-v1` attempt passed component tests but stopped at kernel
lint on an unnecessary generic frame-pointer cast. Passing the already typed
frame fixed inference before `focused-v2` built the image. The first image
selected for physical qualification passed fresh native/kernel emitted reviews.
Its first ordinary TCG boot failed qualification after five actual syscalls,
while the independent peer and complete owned cleanup passed.
No open/read success is inferred from that failure. The original failed scope
and a separate hardware-breakpoint diagnostic of the same retained image remain
in `physical-tcg-v1` and `held-debug-v1`.

The diagnostic showed that the fourth call opened an absent hwcaps candidate,
then the fifth call queried its directory with a trailing slash. Canonical
portable lookup rejected the slash form, and the Linux adapter retired it as
unsupported. This is Linux path translation work: the complete immutable
namespace can prove that missing directory absent without fabricating metadata
for present directories. Failure diagnostics now emit the actual trace after
cleanup, explicitly marked unqualified, so later gaps remain attributable.

The corrected `focused-v3` build passed 66 protocol, 369 host and 611 kernel
unit tests, lints and fresh emitted review. Its ordinary TCG96 run recorded the
first successful Linux open and read: fd 3, 832 bytes from libc offset zero,
with staged and actual caller bytes matching the pinned source prefix. The
normal scenario reached the next unsupported request, syscall 17,
`pread64(fd=3, count=840, offset=64)`, after eight calls and seven returns.

That complete three-lifetime run still failed: scenario 1 attempted to reuse
the boot-wide one-shot PIT rearm operation. `physical-tcg-v2-failure` retains the
actual partial transcript; it is not a qualified three-scenario pass. The
correction rearms only on the first lifetime and leaves the running PIT cadence
and monotonic boot timebase intact thereafter. Every new lifetime is fully
staged before user entry; this proof's timer authenticates and restores the
same task rather than rescheduling or charging a quantum. No shared interrupt
API or one-shot latch is weakened.

The final `focused-v4` build passed 611 kernel tests, strict kernel lint and
fresh exact-image review. Native ELF and deployment bytes remain identical to
v3; only the kernel runtime changed. The final 488-file source freeze is
`source-freeze-v4.json`. Static review closed 238 reachable kernel functions
with no unresolved transfer or cycle. Bootstrap use is 37,200/65,536 bytes,
leaving 28,336 bytes against the required 16 KiB reserve. Gate/syscall peak is
5,424 bytes, full recoverable fault 5,328 and timer 3,312, including entry
frames. Native stack remains 2,696/8,192 bytes.

`physical-tcg-v3` passed at 96/320 MiB with the exact reviewed image. Each
72-record transcript contains three fresh service incarnations and complete
ownership restoration between lifetimes:

| Scenario | Linux calls / returns | Native calls | Delivery | Portable state |
| --- | --- | --- | --- | --- |
| Normal | 8 / 7 | 772 | 832 bytes committed | fd 3, offset 832, unfenced |
| Fault after stage | 7 / 6 | 768 | 832 bytes staged, caller unchanged | offset 0, no read commit |
| Unknown local publication outcome | 8 / 7 | 769 | 832 bytes already committed | prior service watermark 6, handles cleared, fenced |

Each lifetime separately rechecks the full 2,190,608-byte source, then performs
one bounded 832-byte service read, eight inert rejected stage probes and four
inert rejected completion probes. Staged bytes match the exact libc prefix in
every scenario. Actual caller bytes match it in accepted scenarios 0 and 2;
scenario 1 preserves the prior
caller bytes. Faults are at the two retained UD2 symbols, `0x400054` and
`0x400056`; they retire the dependent pair, erase private/staged state and let
the independent peer complete. The third scenario deliberately injects an
unknown *local* publication result after a valid physical acknowledgement; it
does not claim a malformed kernel acknowledgement was delivered.

`physical-kvm-v1` also passed at 96/320 MiB. All four boots used the identical
reviewed kernel/image and pinned native, plan and source bytes. The independent
host oracle accepted every complete transcript, including actual saved frames,
copyout bytes, descriptor state, fault addresses and restored ownership.

Final image SHA-256:
`e4d7b2992ee07851c24dd9e7f38528144e9c86bec44aaf7a79160495d17632e4`.
Kernel ELF SHA-256:
`401c70c7e711c1e1919e521fb5732d6cfc3d2f4b01c0e3cdfb1212e0c9549e02`.
Native ELF SHA-256:
`44190f390b08559293a36f56bd7be42e57538240f7bf15901c9f888562e70669`.

Implementation checkpoint `beeb190` was committed and pushed after focused
checks and all four complete QEMU boots. The remaining qualification below
was completed before the final documentation checkpoint.

## Final qualification

All six retained negative provisioning cases passed: missing, truncated and
single-byte-corrupted main and libc sources are rejected before a user boundary.
These serial receipts establish refusal, not independent physical cleanup
accounting for provisioning failures.

VirtualBox 7.2.6 at 320 MiB booted the identical image, refused
`application_cpu_amd_error_pointers` before userspace, and was powered off. The
retained receipt explicitly has `semantic_qualified: false`; its nonzero test
result is preserved. The QEMU-specific source-provisioning route is not thereby
qualified on VirtualBox.

All 29 required/affected commands and the final CPU workspace image restoration
passed under the same serial memory guard. The final source check retains all
488 exact non-Markdown files; documentation updates do not change the tested
implementation. ADR 0087 is accepted only for this bounded integration.

The exact baseline review found 9 changed bytes in the editor image,
accounted for by one decoded Rust panic location record and the recomputed
Threshold checksums. The source statements, file names, columns, complete machine
instructions, image geometry, embedded native assets and all other bytes remain
identical. The other four baseline images are byte-identical. This is an exact
artifact comparison, not permission to ignore general metadata changes.

Across 21 retained guarded scopes and 86 attempted jobs, observed
peak scope memory was 4,297,056,256 bytes (4.002 GiB), with at least
19,333,636,096 bytes (18.01 GiB) available on the host.
The 4 GiB soft limit caused reclaim during the matrix; no hard-limit event,
OOM kill or pressure abort was observed. All builds/tests/VMs were serialized
with one Cargo job and one test thread. Accessible unit journals, complete
runner endings and sampled host/cgroup counters support these observations;
collected unit status is unavailable rather than historical zero.

Final retained receipts:

- `final-audit-v2.json`: `f44bf4641d729a97bcb8cdb21a6c7a8df1e304e5c71c88ded369abd6a9134f67`.
- `memory-audit-final-v1.json`: `61b4d29657ae329ec48a2957ab3c41536e9502f2e2a6e4d4633b4b8154f9cbc5`.
- `baseline-review-v1/receipt.json`: `80b5216000857898674fd7960d29d585d8e307403ffa1bb021c990f8341a560d`.
- `kernel-emitted-review-v3.json`: `8d98ee95c7625f25e06f7f026fb0ffef4b9bc5011fe8e1f65ae2ffe8cd43943d`.
- `kernel-emitted-semantics-v4.json`: `6745768377021db36f34fb7c5379886010bfbc3a96f0bdb3137d19cf13c8991c`.
- `native-semantics-v4.json`: `c350866fb2f6947f4b87268a9cab27e2a712c8192529809c20121bc6699c5551`.

## Next observed gate

Implement the observed positional read from the same retained libc snapshot,
without advancing fd 3's sequential offset of 832. The real loader requested
840 bytes at offset 64. Its following metadata, mapping/protection and runtime
requests must then be observed and qualified in turn. This checkpoint does not
map libc into Linux, enter main `true`, complete a Linux exit, or launch the
existing Antimatter/Electron application.
