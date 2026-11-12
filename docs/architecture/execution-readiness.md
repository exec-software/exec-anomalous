# Native execution and compatibility readiness

## Qualified native compiler-to-manifest admission (2026-11-11)

Accepted [ADR 0103](../adr/0103-admit-compiler-produced-manifests-in-a-native-loader.md)
connects complete compiler output to manifest admission in ring 3. An explicit
compiler option preserves authored context while omitting generated external
registration. The loader reads an immutable package, rederives identity, intent
and authority, and applies an explicit workload ceiling. Bootstrap delivery
grants and diagnostic reports do not become workload authority.

All 24 final qualification jobs passed on 624 unchanged source files. The same
image passed TCG and AMD KVM at 96/320 MiB: four admissions, six semantic refusals,
twelve reads and one witness-gated loader marker per boot, with complete cleanup.
Three provisioning refusals per backend also passed. The existing delivery
profile passed fresh TCG/KVM regressions, and Linux remains at its unanswered
sequence-12 mmap boundary. Eight focused compiler tests passed.

The measured 18,874-byte loader uses five RX pages, a 128 KiB arena and a guarded
32 KiB stack. The reviewed stack sum is 13,424 bytes for this exact artifact.
The kernel shares the qualified ownership/delivery lifecycle and does not parse
NIL. No NIL edition executes yet; interpreter corpus qualification and mediated
execution are next. The
[Native Manifest Loader Log](../../notebook/Development/2026-11-11%20Native%20Manifest%20Loader%20Log.md)
records artifact identities, limitations and retained attempts.

Reproduce the retained proof with `make native-manifest-smoke`.
`make native-manifest-image` builds its inspected image and external packages;
`make native-manifest-inspect` inspects the loader artifact. The installed SDK
is unchanged; fixture provenance identifies the separately qualified compiler.

## Previous host program-manifest admission (2026-11-11)

Accepted [ADR 0102](../adr/0102-admit-bounded-program-semantic-manifests.md)
implements a shared `no_std` derivation and admission component with host
pack/inspect commands. The bounded textual NIL profile derives console
authority from every accepted function, retains one plain authored main intent
and binds both to the exact edition's SHA-256. An unsigned `.re` envelope must
match independent rederivation and an explicit workload ceiling. Its borrowed
admission witness is a host policy result, not a live native grant.

All 25 final jobs passed on their first attempts against 608 unchanged source
files. Host suites passed eighteen manifest, nine focused tooling, 644 default
kernel, 727 all-feature kernel, 411 xtask and 35 native runtime tests, together
with 1,659 mutation refusals and twenty CLI cases. Fresh TCG native and Linux
regressions passed at 96/320 MiB; the native image is byte-identical to qualified
edition-delivery baseline `914d942`, and Linux's sequence-12 boundary is
unchanged. This increment reran TCG only; the earlier TCG/KVM native evidence
remains separate below. The
[Program Manifest Admission Log](../../notebook/Development/2026-11-11%20Program%20Manifest%20Admission%20Log.md)
records the separate evidence.

Positive fixtures are hand-authored NIL checked by the SDK context inspector.
Complete compiler outputs remain negative fixtures because their external
`nilaix` registration helper is outside the admitted profile. That checkpoint left compiler integration, native admission and interpreter
qualification open. ADR 0103 above now supplies the bounded compiler bridge and
native admission; interpreter corpus qualification remains next. This increment executes no edition and changes no
Linux execution boundary or stable public ABI.

## Qualified native program edition reads (2026-11-11)

Accepted [ADR 0101](../adr/0101-deliver-bounded-native-program-edition-bytes.md)
adds bounded positional reads from one controller-bound immutable edition to
the experimental ANPG gate. Complete destination preflight precedes source
access, exact copied bytes precede receipt commitment, and EOF consumes a
sequence without copying. Live task/handle checks, unchanged destinations on
rejection, scratch clearing and explicit source release bound the service.

The measured client is 5,605 bytes, admitted as two RX pages only through the
named program profile. Its 64 KiB arena and the native application's eight RX
pages and 32 KiB arena retain their bounds. Preparation ends its allocator
borrow before guest entry; teardown validates the resumed continuation before
fresh allocator acquisition. The service probe checks reacquisition and nested
denial while preserving the complete saved frame.

All 30 final jobs passed on 598 unchanged source files. The same reviewed image
passed at 96 MiB and 320 MiB under TCG/Nehalem and AMD KVM/host: seven committed
edition reads, seventeen rejections, 8,203 delivered bytes including repeated
ranges, preserved caller frames and complete source/allocator restoration per
successful boot. Three provisioning refusals per backend also passed. Host
suites passed 644 default-kernel, 727 all-feature-kernel, 402 xtask and 35 native
runtime tests; focused program and tooling suites passed 24 and 12 respectively.
Evidence, artifact identities and preliminary attempts are recorded in the
[Edition Read Log](../../notebook/Development/2026-11-11%20Native%20Program%20Edition%20Read%20Log.md).
The source contains synthetic binary bytes. Native semantic-manifest integration
and the selected Rust interpreter's corpus qualification remain open; the host
admission increment is recorded above. Linux remains
at the unanswered sequence-12 boundary recorded below; no additional Linux
execution or stable public ABI is established.

## Qualified native program console baseline (2026-11-11)

Proposed [ADR 0100](../adr/0100-qualify-native-program-console-transport.md)
connects the ANPG console grant to a bounded `int 0xf4` service. The retained
live task/handle binding admits a complete owned-memory copy, validates UTF-8
and control characters, and commits sequence/length only after synchronous
publication. Rejections preserve output and sequence; exit revokes the binding.
The native fixture verifies two exact messages totaling 60 bytes, sixteen
rejected requests across two arena bases, checked application replies and full
cleanup. All 27 final jobs passed on 503 unchanged source files, including the
96 MiB and 320 MiB proofs under TCG/Nehalem and AMD KVM/host with an identical
image. Exact evidence and failed attempts are recorded in the
[Native Program Console Log](../../notebook/Development/2026-11-11%20Native%20Program%20Console%20Log.md).
The isolated change is based on upstream `main` at `87e365d`, retaining the
existing Ny input, visible-failure and address-space-yield work together with
the Linux loader-input re-pin. Qualification covers that combined baseline;
the earlier development-checkout proof remains separate preliminary evidence.

No edition is mapped or executed by this proof. The
[Runtime Owner Reply](../../notebook/Development/2026-10-15%20NIL%20Runtime%20into%20Anomalous%20%E2%80%94%20Runtime%20Owner%20Reply.md)
selects a bounded Rust interpreter with differential corpus qualification.
The qualified baseline above adds bounded edition delivery; ADR 0093's native
semantic-manifest integration remains open. The pre-existing handwritten
interpreter is preserved unlinked;
its permissive parsing does not establish application readiness. The service is
experimental and does not stabilize the native application ABI.

The latest Linux regression preserves sequence-12
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)` at both RAM sizes, after
eleven returns. This is an unanswered interpreter request, before main entry;
the historical length was `0x198000` before the package re-pin.
[ADR 0098](../adr/0098-model-source-backed-executable-replacement.md) adds a
host-only permission model. Live retained replacement for the current request,
hardware execution and protection evidence, and an authenticated reply are
still required. The native ANEL ELF route and Linux image planning are
implemented subsets, while dynamic linking, main executable entry and Linux
exit remain separate gates.

## Existing Linux execution evidence

Current product direction (2026-10-07):
[ADR 0073](../adr/0073-run-linux-antimatter-through-the-linux-veil.md) selects the
existing Linux Antimatter build through the Linux veil. No native Antimatter
frontend port is required. The dated readiness audit below records prerequisites;
its narrow native/ELF proofs do not establish ordinary Linux binary support.

[ADR 0074](../adr/0074-plan-linux-images-and-initial-stacks.md) now supplies
opt-in, allocation-free Linux ELF64 multi-segment/PIE planning and initial-stack
construction. Host tools inspect unchanged `true`, interpreter, libc and
Electron bytes without executing them.
[ADR 0075](../adr/0075-prove-owned-multi-region-launch.md) supplies a separate
native nine-region proof with exact initialized executable intervals, high user
stacks, missing supervisor ancestry and retained source/permission ownership.
Its synthetic GPR-only tasks do not run the inspected Linux images.
[ADR 0076](../adr/0076-prove-owned-loader-sources.md) implements copied and sealed
native sources that survive producer retirement, qualified under TCG/KVM and
VirtualBox as recorded in the [Owned Loader Sources Log](../../notebook/Development/2026-10-07%20Owned%20Loader%20Sources%20Log.md).
[ADR 0077](../adr/0077-prove-task-owned-fs-bases.md) now qualifies native
per-task FS bases across entry, timers, fault-to-peer return and retirement,
with actual FS-relative memory checks under TCG/KVM and VirtualBox. All sixteen
required checks and ten affected regressions passed; the
[Task-owned FS Bases Log](../../notebook/Development/2026-10-07%20Task-owned%20FS%20Bases%20Log.md)
records the source/artifact and memory evidence. This isolated GPR proof retains
zero GS/KERNEL_GS bases and disables FSGSBASE. Existing application/Linux
profiles retain their zero-TLS requirements.
[ADR 0078](../adr/0078-compose-linux-fs-and-eager-cpu-ownership.md) separately
qualifies the opt-in three-task Linux FS/eager-CPU composition. Linux option
numbers and errors remain userspace policy; native staging binds an FS update
to the exact suspended caller and scheduler reply. Actual FS canaries, selected
caller FP witnesses, timer entry/return, cooperative switching and fault/exit
cleanup are covered. All three eager images start clean and peers retain zero
FS; this fixture does not seed distinct live FP patterns in both peers.

TCG/KVM 96/320 MiB boots, bounded captures and all 28 required/affected targets
passed. VirtualBox refused the existing `application_cpu_amd_error_pointers`
capability gate before userspace and was powered off. It is not a successful
Linux TLS guest proof. The
[Linux FS and Eager CPU Log](../../notebook/Development/2026-10-07%20Linux%20FS%20and%20Eager%20CPU%20Log.md)
retains the qualification receipts.

[ADR 0079](../adr/0079-reach-an-unchanged-linux-interpreter-service-boundary.md)
records the previous one-boundary checkpoint, connecting unchanged `true` and
its interpreter to owned mappings and a real
Linux initial stack. A separate privileged QEMU input supplies retained source
bytes; the host constructs the native plan and ELF interpretation stays outside
the kernel. That default-off proof ran the unchanged interpreter beside a
native peer and userspace veil.

TCG and KVM runs at 96/320 MiB reached the interpreter's genuine syscall 12,
`brk(0)`, at `0x600284c9`. The userspace veil selected unsupported retirement
with `-ENOSYS` result bits. The caller was never resumed and no Linux exit was
observed; the three tasks and their source, mapping, TLS and CPU owners were
retired with resource restoration. This establishes interpreter execution to a
service boundary, not execution of the main `true` entry or completion of
`true`. The TCG/Intel and KVM/AMD runs used identical retained kernel/image bytes.
Missing input, wrong length and wrong hash were rejected in the provisioning
tests. VirtualBox booted but refused `application_cpu_amd_error_pointers` and
was powered off; this is not a successful VirtualBox execution proof. The
29-target regression run, additional fresh CPU-workspace regression and final
evidence audit passed. Independent baseline reviews explain every changed image;
no OOM was observed. The
[Linux Loader Entry Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Entry%20Log.md)
retains the exact artifacts, failures, memory records and qualification limits.

[ADR 0080](../adr/0080-resume-an-unchanged-linux-interpreter-after-a-userspace-reply.md)
records the preceding increment of the same opt-in profile: one userspace reply and a
second genuine syscall. TCG and KVM at 96/320 MiB passed with identical image
bytes. The veil answered `brk(0)` with the plan-derived current break,
`0x40009000`; the complete saved-frame comparison shows only RAX changed.
The unchanged interpreter resumed and reached syscall 21, `access`, at
`0x60029489`, with mode 4 and pointer `0x60033720`. The retained read-only
interpreter bytes at that pointer contain `/etc/ld.so.preload`.

The second call was recorded and retired with userspace-selected `-ENOSYS`
bits; those bits were not returned to Linux. The thirteen-record proof requires
two raw calls, one caller return, seven native calls, three retired tasks and
complete source/mapping/TLS/CPU cleanup. Provisioning tests again rejected all
three malformed-input cases. VirtualBox booted but refused
`application_cpu_amd_error_pointers` before userspace and was powered off. The
29-target regression run and final evidence audit passed; all five development
baseline images remained byte-identical and no OOM was observed. The
[Linux Loader Reply Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Reply%20Log.md)
retains the final qualification evidence and the reviewed audit-label correction.

[ADR 0081](../adr/0081-answer-an-unchanged-loader-file-probe-in-userspace.md)
qualifies the preceding preload-file reply. Existing native operation 18 copies the
actual suspended caller pathname under sealed authority; the userspace veil
returns `-ENOENT` for that exact, explicitly declared absent name. Six rejected
native copy probes and nineteen successful byte reads precede the second
RAX-only reply. The unchanged interpreter resumes to a third real syscall,
`openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY | O_CLOEXEC, 0)`, where it is
retired without another Linux return. The third pathname is reconstructed from
retained source, not copied by the guest in this proof.

TCG/KVM at 96/320 MiB, three negative provisioning boots, all 29 regression
targets and the final evidence audit passed. The fifteen-record proof covers
three raw calls, two returns, 34 native calls, 25 copy attempts (nineteen
successes, six rejections), cleared scratch and complete retirement of all
three owners. All five baseline images remained byte-identical, all 482 frozen
source files remained unchanged and no OOM was observed. VirtualBox's CPU
capability refusal and poweroff remain separate. The
[Linux Loader Access Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Access%20Log.md)
records final qualification and evidence limits.

[ADR 0082](../adr/0082-answer-the-loader-cache-probe-with-fresh-caller-authority.md)
advances to three actual Linux returns. The private v3 userspace policy explicitly
declares the cache absent too. Fresh sealed caller authority supplies seventeen
copied cache bytes, after six additional rejected copy probes. Separate reply
journals and verified clearing prevent the prior pathname from being reused as
evidence. The kernel still does not interpret either Linux name or result.

All four TCG/KVM 96/320 MiB runs reach a fourth `openat` at `0x60029743`,
with pathname pointer `0x7fff0000eec0` inside the writable stack. That pathname
was not copied and cannot be reconstructed from the original source. The fourth
request is retired without a Linux return. Seventeen ordered records prove
three RAX-only replies, four raw calls, 59 native calls, 48 copy attempts
(36 successes and twelve rejections), cleared scratch/journals and complete
three-task resource restoration. All 29 regression targets passed, including
the repaired presentation capture waiter. Negative source provisioning and the
final evidence audit passed; all five baseline images remained identical and
no OOM was observed. VirtualBox's CPU capability refusal and poweroff remain
separate. The
[Linux Loader Cache Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Cache%20Log.md)
records qualification and the retained first presentation failure.

[ADR 0083](../adr/0083-observe-the-loaders-live-pathname-before-file-service.md)
now qualifies authenticated observation of that live pathname. All four
TCG/KVM 96/320 MiB runs copied
`/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libc.so.6` from the owned stack:
55 bytes including NUL, with the complete terminal caller frame unchanged.
Each eighteen-record proof reports four raw calls, three returns, 120 native
calls and 109 copy attempts (91 successes, eighteen rejections), then complete
retirement. This measures the actual request without assigning namespace
meaning or returning a fourth result. The 256-byte pathname capture budget introduces
no global native path or depth limit.

All 29 required/affected targets, negative provisioning and the final evidence
audit passed. All five development images remained identical, all 482 frozen
source files remained unchanged and no OOM was observed. VirtualBox refused
the existing CPU capability gate and powered off. The
[Linux Loader Live Path Log](../../notebook/Development/2026-10-11%20Linux%20Loader%20Live%20Path%20Log.md)
records the independent reviews, exact captures, corrected lint failure and
memory/evidence limits.

Accepted [ADR 0084](../adr/0084-model-read-only-userspace-file-services.md)
qualifies the portable userspace file-service foundation on the host. The default-off `read-only-files` module declares a complete scoped
namespace of exact byte names, distinguishing absence from out-of-scope or
unsupported requests. Borrowed immutable object/version bytes remain pinned
through task-incarnation descriptor lifetimes, including replacement of the
same name in another view. Open, bounded sequential read and close update local
state only after a synchronous publication callback commits; rejection preserves
committed state, while an unknown outcome or callback unwind revokes the service.
Explicit per-instance budgets introduce no global path length or depth limit.

The host qualification binds held libc bytes at the base library name
and declares the observed hwcaps candidate absent within its configured view.
That declaration does not change the running guest namespace. The existing v3
guest policy still declares only preload/cache absence, and the ADR 0083 guest
baseline still returns exactly three results before capturing and retiring the
fourth request. No fourth guest return or successful guest file open is added.

Both independent descriptor tables delivered all 2,190,608 libc bytes exactly;
the host verifier checked all 2,739 read publications. All 29 regression commands,
fresh TCG/KVM 96/320 MiB proofs and the final evidence audit passed; five baseline
images remained identical. The guard stopped an earlier host-pressure spike,
then a complete retry passed with unchanged limits. No OOM was observed.
VirtualBox's existing capability refusal remains separate. The
[Read-only File Service Log](../../notebook/Development/2026-10-11%20Read-only%20File%20Service%20Log.md)
records both host and guest evidence and their limits.

[ADR 0085](../adr/0085-native-retained-file-source-reads.md) adds a separate
retained file source to the default-off guest proof. The native veil reads the
complete pinned libc in bounded chunks while invocation 4 stays suspended.
The source adds 535 owned pages and one lease, without changing the launch plan,
its 512 KiB source budget, the 88 Linux user pages or the existing three replies.
A distinct native deployment binding leaves ANLPOL03 unchanged. The kernel
checks both its delivered stream and a report derived from bytes consumed by
userspace. Ten rejected reads preserve destination witnesses. The
[Native File Source Log](../../notebook/Development/2026-10-11%20Native%20File%20Source%20Log.md)
records qualification and exact remaining limits.

TCG/KVM at 96/320 MiB and all 29 required/affected commands passed. Six negative
provisioning cases refused before userspace entry. The final audit preserves an
earlier pressure abort and a narrowly reviewed diagnostic-only editor image
change; no OOM kill was observed. VirtualBox still refuses its existing CPU
capability gate and does not qualify this source-read path.

The portable file-service model now also borrows an immutable source provider
under [ADR 0086](../adr/0086-read-immutable-file-sources-in-bounded-chunks.md).
Bounded reads use caller-owned scratch and per-invocation context, with exact
delivery required before publication. The default slice API shares its offset
planning and commit path. Scratch clearing is logical, with no claim of volatile
erasure or abort-time cleanup. That portable checkpoint required a native
adapter for the retained source operation.
Independent host verification reconstructed the pinned libc for two clients
and checked 2,755 operations. All 29 required/affected commands passed; fresh
TCG/KVM 96/320 MiB runs preserved the earlier native proof and all five baseline
images remained identical. The
[Bounded File Backend Log](../../notebook/Development/2026-10-13%20Bounded%20File%20Backend%20Log.md)
retains exact source, transaction, artifact and memory evidence.
The native integration under
[ADR 0087](../adr/0087-return-native-file-service-replies-to-the-linux-loader.md)
now returns a successful libc open and read to the unchanged Linux interpreter.
The userspace veil derives missing candidate files/directories from its explicit
complete namespace, opens `libc.so.6` as fd 3, and returns 832 pinned source bytes.
Authenticated caller/service context, staged copyout, actual caller readback and
the delayed exact acknowledgement join portable descriptor/offset publication.
At that checkpoint the next actual unsupported request was
`pread64(fd=3, count=840, offset=64)`; the normal run had eight calls and seven
Linux returns, with offset 832.

TCG and KVM at 96/320 MiB pass the same image's three fresh lifetimes: normal
publication, a fault after staging before publication, and an injected unknown
local result after an accepted read. The dependent pair retires, private/staged
bytes are cleared and an independent peer finishes with restored ownership.
The unknown-result case fences the portable service without replaying the read.
Six negative provisioning cases pass. VirtualBox still refuses its CPU capability
gate before userspace; cleanup powers it off. All 29 required/affected checks and
the final CPU workspace image restoration passed. The baseline review admits
only the editor's diagnostic line-number
changes; other development images remain identical. No OOM was observed. The
[Native File Replies Log](../../notebook/Development/2026-10-13%20Native%20File%20Replies%20Log.md)
retains exact artifacts, failed attempts and claim boundaries.

The positional-read integration under
[ADR 0088](../adr/0088-read-linux-files-without-advancing-descriptor-offsets.md)
now returns the 840-byte ELF program-header range at libc offset 64. The shared
portable read plan keeps the explicit source offset separate from the retained
descriptor offset. An exact delayed acknowledgement advances the sequence
watermark to 8 while the sequential offset remains 832. The real interpreter
then requests `fstat(fd=3)`, syscall 5, after nine calls and eight Linux returns.

TCG and KVM at 96/320 MiB qualify identical reviewed image bytes across five
fresh lifetimes: normal delivery, both existing sequential-read fault cases,
and corresponding positional-read cases. Two retained delivery journals keep
earlier observations intact; independent host verification compares each actual
source range, caller bytes and saved frame. Six source-provisioning refusals
pass. VirtualBox still refuses its CPU capability gate before userspace and is
powered off. All 29 required/affected checks and final CPU workspace image
restoration pass; all five development baseline images remain byte-identical.
Peak guarded memory is 2.60 GiB, with no observed OOM or pressure abort.
The [Positional File Reads Log](../../notebook/Development/2026-10-13%20Positional%20File%20Reads%20Log.md)
records the implementation, exact evidence and retained failed attempts.

[ADR 0089](../adr/0089-publish-open-file-metadata-through-the-linux-veil.md)
now qualifies open-file metadata and the following repeated positional read.
The portable service publishes retained object/version/length without provider
I/O or offset changes; the Linux adapter explicitly encodes all 144 stat bytes,
returns zero and requires a distinct copied-length acknowledgement. The unchanged
interpreter then reaches syscall 9, `mmap(NULL, 2231696, PROT_READ,
MAP_PRIVATE | MAP_DENYWRITE, 3, 0)`, after eleven calls and ten Linux returns.
That mapping is unsupported and retired without a Linux return or exit.

TCG/KVM at 96/320 MiB pass identical reviewed artifacts across seven lifetimes,
with four normal delivery journals, per-delivery rejection probes and complete
fault cleanup. The first three-journal image failed at the repeated read; its
evidence remains distinct. Fresh review and independent physical verification
qualify the correction. All six provisioning refusals and 29 required/affected
checks pass, the CPU workspace image is restored, and all five development
baseline images are byte-identical. Peak guarded memory is 2.18 GiB with no
observed OOM or pressure abort. VirtualBox retains its CPU capability refusal
and verified poweroff. The
[Open File Metadata Log](../../notebook/Development/2026-10-13%20Open%20File%20Metadata%20Log.md)
records exact evidence and the runtime-mapping handoff.

[ADR 0090](../adr/0090-retain-runtime-mapping-transactions.md) introduces the
next kernel ownership prerequisite: an isolated runtime mapping transaction
that retains a complete virtual extent, its populated prefix and the backing
owner. Publication rejection permits retry; accepted or uncertain publication
requires explicit withdrawal before cleanup. Fifteen new transaction tests in
the 626-test kernel suite, strict lint and all 29 required/affected regressions
passed. Five development images remain identical; a deadline and a host-pressure
abort are preserved separately, with no observed OOM kill. It adds no live caller
mapping or Linux reply. The
[Runtime Mapping Transactions Log](../../notebook/Development/2026-10-13%20Runtime%20Mapping%20Transactions%20Log.md)
records this separate mechanism and its integration requirements.

[ADR 0091](../adr/0091-release-allocator-borrows-before-userspace.md) now
qualifies allocator access from the authenticated service path. Preparation's
allocator borrow ends before guest entry; every one of seven lifetimes records
two ordinary service borrows and one refused nested borrow. Fresh kernel emitted
reviews, the strict oracle and an independent crosscheck pass identical reviewed
artifacts under TCG/KVM at 96/320 MiB, preserving caller state and fault cleanup.
All thirty required/affected regressions and six provisioning refusals pass.
The final audit retains all failed attempts and records no new observed OOM
kills; soft-limit reclaim occurred without a hard-cap event. VirtualBox's CPU
capability refusal and verified poweroff remain separate. The qualified code
checkpoint is `6e30506`, excluding Claude's later ADR 0094 shared-core work.
The [Allocator Service Lifetime Log](../../notebook/Development/2026-10-13%20Allocator%20Service%20Lifetime%20Log.md)
records exact source, artifact and receipt identities for that checkpoint.

Accepted [ADR 0095](../adr/0095-prepare-and-cancel-authenticated-source-mappings.md)
connects authenticated service allocation to a real retained source and the
caller's actual mapping plan. Native operation 25 prepares, inspects and cancels
an isolated unpublished tree: 535 populated pages, 545 reserved virtual pages,
five table frames, 752 zero-padding bytes and ten absent tail pages. Source
readback, permissions, absent pages, accounting and caller preservation are
checked before cancellation returns the native page counts. One normal
preparation and nine refusals include the rejected replay; the six earlier
fault lifetimes perform no mapping work. The separate allocator 2/1 probe is
preserved and does not count all allocation callbacks.

Fresh kernel and native emitted reviews precede four passing TCG/KVM boots at
96/320 MiB; the strict oracle and independent physical crosscheck agree. All
thirty required/affected regression jobs and six provisioning refusals pass.
The first KVM attempt's guest timeout and the first regression attempt's outer
deadline during kernel lint remain failed attempts. VirtualBox separately refuses
its CPU capability gate and is verified powered off. The
[Live Mapping Preparation Log](../../notebook/Development/2026-10-15%20Live%20Mapping%20Preparation%20Log.md)
records exact identities, remaining evidence limits and the final memory audit.

Accepted [ADR 0096](../adr/0096-retain-publish-and-withdraw-blocked-caller-mappings.md)
qualifies retained source ownership, inactive caller-tree publication, composed
user-copy inspection, and withdrawal with invalidation before cleanup. The
published owner is retained across native gates; uncertain outcomes retain all
resources and bar caller continuation. The selected ten/three circle policy
comes from the actual retained owner, not a user handle or an observed count.

Fresh kernel and native emitted reviews, four TCG/KVM boots at 96/320 MiB,
independent physical verification, six provisioning refusals and all thirty
required/affected regression commands pass. The first formatting attempt remains
failed evidence. VirtualBox separately refuses `application_cpu_amd_error_pointers`
and is verified powered off; it has no loader-execution pass. The final audit
records fifteen guarded scopes and sixty attempted jobs, no overlapping guarded
job intervals and no observed OOM kill. Soft-limit reclaim occurred during
`make check`, without a hard-cap event.
The [Mapping Publication Log](../../notebook/Development/2026-10-15%20Mapping%20Publication%20Log.md)
records the source, emitted review, physical evidence and current baseline image
comparison, including its runtime coverage limits.

At that historical publication checkpoint, the caller remained suspended at
sequence 11 after ten Linux returns. ADR 0097 and the current checkpoint above
subsequently qualify the first mmap return and the next request. The
read-only, non-executable candidate is never selected as a running caller
mapping. This slice adds no post-publication guest fault injection and proves
no Linux mmap return, main `true` entry, dynamic linking, Linux exit or Antimatter
execution. Next qualify hardware-use ownership, safe caller activation and the
exact authenticated userspace Linux mmap reply, then observe the next interpreter
request. Foreign syscall and ELF policy remain in userspace.
Other file operations, dynamic mapping/protection, dependency loading and
TLS/libc startup remain needed for a real `true` exit and the larger
Antimatter/Electron environment. Proof-local work budgets introduce no global
path ceiling. The dated audit below records earlier scopes.

Date: 2026-10-04 (bounded native replacement and ELF comparison qualified)

This audit separates implemented mechanisms from proposed formats and the
remaining execution prerequisites. It does not stabilize an ABI or select wire
fields. The user has selected kernel foundations first, then native Antimatter
running and rendering in userspace under
[ADR 0045](../adr/0045-select-native-antimatter-ny-and-hag-direction.md).
Ny is the official language direction for Anomalous and HAG; HAG is the compute
runtime intended to replace Vulkan. Their native ports remain open. The latest
user decision selected a native ABI baseline followed by an explicitly scoped
ELF path. That bounded userspace loading comparison is now qualified under
proposed [ADR 0051](../adr/0051-prove-userspace-elf-loading-against-native.md).
The Linux copy of Antimatter on the host outside the VM
supplies the behavioral reference; guest Linux startup and a bounded syscall
personality are qualified within proposed ADR 0052's fixed record-device proof.
ADR 0053 implements a separate userspace policy vessel; its bounded three-task
qualification passed 39 matrix jobs and two restorations. ADR 0054 now qualifies
a fixed two-caller shared service through 41 qualification checks and two restorations.
ADR 0055 implements bounded native startup and private arenas; all 43 qualification checks and two image restorations passed. ADR 0056 qualifies the bounded native RAM document service. ADR 0057 qualifies its bounded ELF comparison; all 48 qualification checks and two development-image restorations passed.
ADR 0058 qualifies native editor replacement with the same service retained;
all 50 qualification checks and two development-image restorations passed.
ADR 0059 qualifies the matching bounded userspace ELF replacement comparison;
all 52 qualification checks and two development-image restorations passed.
Windows and macOS comparisons follow later. Antimatter is largely Electron today.
The first native contract proof uses a bounded Rust
client under proposed [ADR 0050](../adr/0050-prove-a-bounded-native-application-contract.md);
it does not attempt a full Electron, Antimatter, or Ny runtime port.

Executable framing, execution ABI, and application services are separate
contracts. PE/COFF plus a Windows personality, Mach-O plus a Darwin personality,
and ELF plus a named personality each need their own admission and runtime
evidence. ADR 0049's Ny ELF object is a native build input, not evidence of
Linux execution. Antimatter is the intended first real multi-ABI application
test: compare matching behavior, resource ownership, surface/input and workspace
service results, and fault/exit cleanup against the native baseline. State
unsupported behavior explicitly; neither readable bytes nor a visible frame
establishes foreign or Electron compatibility.

## What exists

| Boundary | Implemented evidence | Limit |
| --- | --- | --- |
| Native rites | `anomalous-abi` has fixed-width 40-byte call/reply prefixes, explicit rights declarations, bounded payload classifications, and little-endian helpers; accepted ADR 0019 proves task-bound admission. | Experimental operation numbers and schema; declaration is not authority, and this is not a stable application or syscall ABI. |
| Semantic invocation | `invocation_header` carries shared human/machine intent and identity fields. | Its documented milestone-0 representation is target-native; canonical serialization and version negotiation remain open. |
| Fixed native execution | Build tooling inspects linked vessel ELF files; feature/development runtimes copy known payloads into owned mappings and construct internal task bounds. | No versioned general launch descriptor or arbitrary application admission; ADR 0051 separately adds the bounded runtime ELF adapter below. |
| Scheduling and isolation | Accepted proofs cover private roots, generational identities, two PIT preemptions, exact known faults, restart, copied IPC, and bounded wake paths. | These are fixed experiments, not a production dispatcher or general process lifecycle. |
| Memory planning | Kernel-private, unsafe-free planners validate up to eight user regions, eight exclusions, exact executable membership, and a 32-frame user-tree budget; separate typed physical/high-half virtual geometry supports checked range translation. All 19 new tests and 23 regression targets passed. | Metadata only: no leases, pointers, page-table construction, activation, live low-address mapping, or allocation rollback. |
| Owned inactive construction | A retained stage owns the planned table arena and per-region data leases, populates and validates every entry, and supports cleanup retries after partial failure. A temporary supervisor RW+NX high alias supplies privileged access under the bootstrap root. All 24 regression targets passed, including two-size memory proofs under TCG and KVM. | User-only roots remain inactive and contain no resident kernel mappings. The alias is a temporary BSP proof; no permanent layout, low-root activation, global W^X, or general application launch. |
| Resident activation | Proposed ADR 0043 adds explicit supervisor kernel/stack mappings and a retained activation session for two low-user roots; native evidence covers root switching, physical-alias collisions, exact CPL0 permission faults and complete restoration. | Bounded BSP proof only. No permanent layout, ring-3 launch integration, safe arbitrary user-copy, general fault policy, or global W^X. |
| Owned low-address user execution | Proposed ADR 0044 enters two real GPR-only tasks with owned roots, declared initialized-code membership, guarded stacks, four exact resumed reports, B's contained supervisor-read fault, A's exit, and complete lifetime cleanup. All 24 validation targets passed, including 96-MiB/320-MiB memory proofs under TCG and KVM, on 240 unchanged frozen source/configuration files. | Removable nonscheduled fixture with a private handshake. No general native launcher, syscall ABI, arbitrary user-copy, extended CPU-state ownership, or permanent layout. |
| Retained prepared launch | Proposed ADR 0046 composes mapping population and circle attachments without publishing a task, then commits task creation explicitly. `cancel_unstarted` and retained cleanup cover thirteen native failure/cancellation cases before the original two users execute through the owner. All 24 regression targets passed on 254 unchanged non-documentation files, including 96-MiB/320-MiB TCG/KVM proofs. | At that checkpoint, the same three-region GPR-only unscheduled fixture with a trusted population callback; no general launcher, artifact ABI, scheduler integration, or Antimatter execution. |
| Retained-launch sustained dispatch | Proposed ADR 0047 composes two prepared owners, high-alias privilege stacks, unstarted admission rollback, and clear/release retries with 64 preemptions, one exact UD2 fault, and 16 survivor ticks. Focused tests, 96-MiB/320-MiB TCG/KVM proofs, and all 26 targets qualified across documented phases; 258 final frozen files remained unchanged. | Two-task GPR-only feature proof; faulted ownership lasts until both tasks are terminal. No stable ABI, general loader, FPU/TLS ownership, or selected application CPU/copy/fault profile. |
| Internal application profile | Proposed ADR 0048 implements eager x87/SSE2 ownership, current-task copies bounded to 4096 bytes, selected #DE/#UD/#GP/#PF containment, and last-current fault cleanup. All 28 targets passed on 265 unchanged frozen files, including two-size TCG and actual AMD-host KVM proofs. | Trusted removable fixture, not full untrusted-program admission. No TLS, AVX/XSAVE, #MF/#XM recovery, stable ABI, general loader, or Ny startup qualification. |
| Bounded Ny object | Proposed ADR 0049 compiles one scalar source with the pinned SDK and preserves its emitted object, linking an inspected startup shim for initialized data, generated integer/f64 work, timer returns and failure/peer contrast. All 30 targets passed on 272 unchanged frozen files, including two-size TCG/AMD-host KVM proofs. | One fixed object and four narrow runtime hooks; no hosted runtime, general Ny target, arbitrary executable admission, stable application ABI or Antimatter execution. |
| Bounded native application | Proposed ADR 0050 qualifies independent Rust editor/service tasks, canonical copied surface/input packets, seven real Antimatter reference states, terminal revocation and one deferred final presentation. All 33 matrix jobs passed on 291 unchanged frozen files, including two-size TCG/KVM and capture proofs. | Printable ASCII through 31 bytes, one undo entry, scripted commands and a 128×8 software surface. No full Electron editor, Ny frontend, physical input or stable application ABI; that checkpoint used host-extracted payloads. |
| Userspace ELF loading | Proposed ADR 0051 implements a ring-3 parser of one canonical two-segment ELF64 profile, copied 96-byte inert plans, exact trusted-manifest admission, loader reclamation, and native/ELF route comparison. All 35 matrix jobs passed on 302 unchanged frozen files, including two-size TCG/KVM proofs and captures. | Inspected fixture with native `int 0xf4` startup/services. No arbitrary ELF, Linux personality, dynamic linker, TLS, foreign authority or stable loader API. |
| Window content | Native immutable RGB8 frames, granted content owners, clipping, and complete-frame publication are implemented. | Producer and compositor share one trusted workspace vessel; no independent application or foreign graphics protocol. |
| Reliquary and Covenant | Proposed architecture describes native entry/segment records, versioned chunks, governed context, provenance, signatures, and immutable generations. | No implemented Reliquary encoding/parser, signature policy, package resolver, or transactional artifact store. The ELF fixture is a separate format. |
| Linux entry/device subset | Proposed ADR 0052 implements real SYSCALL entry, a minimal initial stack and read/write/poll/yield/exit over two fixed task-bound record devices. All 37 matrix jobs and two restoration jobs passed on 313 unchanged frozen files, including two-size TCG/KVM and captures. | Feature-private proof controller, not a production userspace veil, Linux pipes, libc, signals, TLS or general Linux compatibility. |
| Userspace syscall veil | Proposed ADR 0053 implements an independent policy task, sealed blocked-caller authority, inert staging and fault containment across three retained tasks. Qualification passed 39 matrix jobs and two restorations. | One fixed caller/veil relation; startup remains a controller fixture. No multi-caller production service, arbitrary Linux runtime, Mach-O execution or Darwin personality. |
| Shared userspace veil | Proposed ADR 0054 qualifies five retained tasks, sealed two-caller membership, independent routes, caller survival and shared service-loss containment. All 41 qualification checks and two restorations passed. | Fixed two-member serial service and controller startup; no concurrent request queue, restart, general runtime or full Antimatter/Electron. |
| Native startup and private arena | Proposed ADR 0055 implements a versioned 128-byte copied descriptor, task-bound handles, guarded 32 KiB private arenas, and heap-backed editor state/buffers. All 43 qualification checks and two image restorations passed. | Explicit four-region fixture; no live growth, general launch format, file service, threads/TLS or full Antimatter/Electron. |
| Native RAM document service | Proposed ADR 0056 implements independent editor/service tasks, copied open/read/save/close records, exact delayed-save acknowledgements, stale-handle rejection and uncertain service-loss outcomes. Bounded native qualification passed. | One bounded document; close/reopen retains bytes only in the same live service. No durable storage, owner-replacement recovery, general filesystem or full Antimatter/Electron. |
| ELF document comparison | Proposed ADR 0057 implements the same document trace through native and guest-parsed ELF spans, with verified zero-tail compaction, an independent document wire oracle and compared final pixels. Bounded qualification complete. | Same native personality and fixed inspected client; parser, 96-byte plan and 32 KiB loader bound unchanged. No arbitrary ELF or new persistence claim. |
| Native editor replacement | Proposed ADR 0058 adds destructive editor replacement while retaining the same RAM document service, with fresh authority and an independent 122-record oracle. Bounded qualification passed all 50 checks and two development-image restorations. | Three fixed cases, one replacement per service and unchanged native personality. No service restart, filesystem durability, general process manager or full Antimatter/Electron. |
| ELF editor replacement comparison | Proposed ADR 0059 implements adjacent native/ELF versions of all three retained-service replacement scenarios with an independent 222-record oracle. Bounded qualification passed all 52 checks and two development-image restorations. | Native personality, fixed inspected fixture and live RAM service only; no arbitrary ELF, service restart or filesystem durability. |

The implementation anchors are
[`anomalous-abi`](../../crates/anomalous-abi/src/lib.rs),
[`rite` admission](../../kernel/src/rite.rs),
[`task_bounds`](../../kernel/src/task.rs),
[`workspace_runtime`](../../kernel/src/arch/x86_64/workspace_runtime.rs), and
[`xtask` ELF inspection](../../tools/xtask/src/main.rs). Reliquary's
[package architecture](package-management.md) explicitly labels its design
unimplemented. Accepted [ADR 0019](../adr/0019-define-first-supervised-vessel-rite-abi.md)
explicitly excludes a stable syscall ABI. Therefore a claim that the native
artifact/ABI format is already done would be incorrect.

The existing host ELF inspector requires section and section-name tables and
uses host allocations. Its workspace caller requires exact linked addresses,
two fixed load segments, and no dynamic imports, relocations, or TLS. It is
build validation, not a general runtime ELF loader; it should not become one
by moving the same parser into ring 0.
The separate [`anomalous-elf-adapter`](../../crates/anomalous-elf-adapter/src/lib.rs)
is allocation-free and unsafe-free. The
[`loader vessel`](../../vessels/anomalous-elf-loader/src/main.rs) invokes its ELF
parser in userspace; privileged admission decodes only the inert native plan.

## Kernel prerequisites still open

| Gap | Current restriction | Required later evidence |
| --- | --- | --- |
| Sustained dispatch | ADR 0039 proved 64 switches, one exact fault, and 16 sole-runnable ticks under TCG/KVM. Proposed ADR 0047 composes that behavior with retained prepared launches and owned low roots; focused tests, two-size TCG/KVM, and all 26 regression targets qualified across documented phases. | Bounded two-task GPR-only feature proof, not a production dispatcher or general execution profile; no new VirtualBox evidence. |
| User/kernel address separation | The legacy live mapper reserves all of PML4 slot zero (512 GiB) for a borrowed supervisor branch, although the bootstrap identity map covers only the first 4 GiB. The kernel is linked at physical/virtual 1 MiB, and allocator/table access uses physical addresses as pointers. | ADR 0043 demonstrates a bounded low supervisor island plus an independent high alias under two active low-user roots. Durable placement and migration of the live runtime remain open; raising capacities or clearing a reserved-slot check is insufficient. |
| User mapping geometry | The legacy live address-space manager still admits three extents in one leaf-table branch. ADRs 0042/0043 separately prove retained multi-branch ownership and CPL0 activation; ADRs 0044/0046 exercise two real ring-3 task/circle lifetimes and retained launch cancellation. | General failure-atomic launch and legacy-runtime migration remain open. The bounded three-region user fixture does not make arbitrary admitted plans runnable. |
| Executable return bounds | ADR 0075 adds an opt-in exact initialized interval set to `task_bounds` and entry/return admission, preserving holes between main/interpreter-shaped regions. Legacy profiles retain their single-interval constructors and fixed-runtime checks. | General loader admission and migration of legacy profiles remain open; the nine-region fixture does not make arbitrary application layouts runnable. |
| Execution state | Earlier fixtures are GPR-only with CR0.TS set. ADR 0048 qualifies an isolated eager x87/SSE2 owner and exact kernel-state restoration under TCG and AMD-host KVM; ADR 0049 qualifies one generated Ny scalar object with actual timer returns. Zero FS/GS/KERNEL_GS bases remain required. | Additional Ny code/runtime needs require explicit qualification; TLS, AVX/XSAVE and other extended state need separate ownership and evidence. |
| User memory access | ADR 0048 qualifies complete owner/identity/permission/page-path preflight for nonempty copies up to 4096 bytes through retained supervisor aliases, including 20 successful/40 rejected operations per native run. | This is tied to the retained three-region owner and sole-BSP exclusion. Arbitrary hardware-fault recovery, concurrent mutation and a general foreign pointer ABI remain unqualified. |
| Fault and lifecycle policy | ADR 0048 qualifies selected CPL3 #DE/#UD/#GP/#PF attribution separately from return admission, including invalid-RIP/RSP terminal fixtures and last-current fault cleanup. | #MF/#XM and other unsupported classes remain fatal, as do CPL0/nested or corrupt-owner failures. This is not a complete untrusted-application fault profile. |
| General launch | Native payloads and grants remain fixed by the feature runtime. ADRs 0046/0047 retain preparation, commit, cancellation, dispatch and rollback; ADR 0048 adds the internal CPU/copy/fault profile and ADR 0049 qualifies one Ny object/startup composition. Preparation still publishes no task. | ADR 0050 qualifies a bounded surface/input prototype; full editor/workspace services remain open. The fixed serialized ELF load plan grants no authority; a stable general launch descriptor and arbitrary executable admission remain open. |

The mapper restrictions are visible in
[`address_space`](../../kernel/src/arch/x86_64/address_space.rs), the kernel
placement in [Threshold's kernel linker script](../../boot/threshold/x86_64/kernel.ld),
and fixed return checks in
[`scheduler_test`](../../kernel/src/arch/x86_64/scheduler_test.rs). Current context
limits and generic-runtime exclusions are recorded in the
[kernel architecture](kernel.md). Removing one restriction does not remove the
others. The new legacy FP/SSE2 profile remains separate from earlier GPR-only
fixtures and from broader CPU-state support. No single proof can mark
the whole kernel ready for arbitrary Linux programs.

The low-address issue is an ownership hazard, not only an address-range
restriction. While a user root is active, remapping a low virtual page can
redirect a kernel access that casts the same physical address to a pointer.
The kernel, page-table walker, allocator clearing/copy paths, interrupt/fault
entry, and saved transition state must retain valid privileged access while
user virtual mappings vary. The bounded resident proof now separates user
geometry from privileged physical access and exercises two actual mixed roots.
It retains only explicit 4 KiB supervisor sections and stacks below 2 MiB, plus
a checked high alias of Threshold's reserved physical hierarchy. The permanent
placement decision remains open. This bridge does not migrate the legacy live
mapper, remove its PML4-slot-zero reservation, or provide global W^X: the
supervisor RW+NX alias can still write executable physical storage.

## Modular implementation order

1. **Preserve the qualified native application contract.** Build on retained launch and
   ADR 0048's internal CPU/copy/fault profile. Proposed ADR 0050 selects a bounded
   Rust client, explicit copied surface/input bytes, and task-bound channel
   authority. Test document/selection/undo/appearance behavior and lifecycle
   without treating the client as a full Antimatter or Ny port. Independent
   grants, checked extents, non-W+X mappings, and retained ownership remain
   prerequisites; requested rights never grant themselves.
2. **Extend the qualified userspace veil, then expand supervised runtime services.** Proposed ADR 0051 qualifies a bounded
   sectionless x86-64 profile and the existing native `int 0xf4` personality.
   Parsing produces an inert plan and grants no memory, task or execution
   authority. Kernel launch checks the copied plan against the trusted manifest
   and retains all ownership; it does not parse ELF. Proposed ADR 0052 now
   qualifies Linux initial process state, SYSCALL entry and a fixed record-device
   adapter inside the feature-private proof controller. ADR 0053 implements
   runtime policy in a separate veil. ADR 0054 qualifies fixed two-caller
   membership and service-loss containment. ADR 0055 implements copied native
   startup and private arenas, with bounded qualification complete. ADR 0056
   qualifies bounded RAM document open/save. ADR 0057 qualifies its bounded ELF
   comparison. ADR 0058 adds native editor replacement while retaining the RAM
   service; bounded qualification is complete. ADR 0059 qualifies that same
   bounded replacement through the userspace ELF loader against the native
   baseline. ADR 0063 subsequently qualifies bounded native disk persistence,
   fault cuts, lost-reply reconciliation and cold recovery; general filesystem
   services remain open. Signals, threads,
   dynamic linking and general runtime behavior remain exclusions.
3. **Compare matching behavior against the host reference.** Run the Linux copy
   of Antimatter outside the VM as a reference for document, selection, undo,
   appearance, and later workspace behavior. Compare only common supported
   operations and state, with ownership and cleanup evidence from each guest
   path. A host test is not an Anomalous Linux/Electron execution result.
4. **Extend comparisons to Windows and macOS later.** Qualify Windows PE/COFF
   with a selected Windows personality, and macOS Mach-O with a selected
   Darwin/Mach/BSD personality. Both translate through the same native ownership
   and service boundaries. Their independent ADR 0008 feasibility gates remain;
   success with one format or personality establishes no other compatibility.

Reliquary framing and Covenant packaging remain independent follow-ups. A
constrained loader can eventually produce the same native launch plan from a
versioned execution envelope; signatures, governed context, package activation,
and serialized launch fields require separate decisions and evidence. A fixed
build artifact can precede that envelope. HAG acceleration and its native device
transport remain separately gated. See the
[direction log](../../notebook/Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
for the audited gaps and progression.

“Final stop” describes the intended common native boundary for adapters here;
it is not the name of an implemented protocol. Linux-specific, Windows-specific,
and Darwin-specific structures must not leak into the common kernel contract.
The existing native rite and semantic invocation contracts must also state their
own serialization, versioning, authority, and compatibility policies before any
stable-ABI claim.

Proposed [ADR 0040](../adr/0040-separate-entity-identity-authentication-and-authority.md)
and the [identity design](identity-and-authority.md) add an authentication
constraint on that launch boundary: an entity name, credential assertion,
artifact signature, or foreign UID must not manufacture execution authority.
An authenticated user-space service must bind admitted authority to the exact
live native task/channel and retain its revocation semantics. Future biometrics
verify a human locally for key use; they add no biometric fields or special
administrator identity to the kernel ABI. This binding remains unimplemented.

## Completed bounded native document increment

Proposed [ADR 0056](../adr/0056-prove-task-bound-native-document-open-and-save.md) implements an independent
RAM document/render service and editor over four task-bound copied routes.
Only a matched semantic save reply changes the submitted snapshot's baseline;
transport acknowledgement alone proves no document effect. Newer edits survive
a late reply. Unknown outcomes force dirty/unsynchronized presentation even
when the current text equals the last acknowledged baseline.

Bounded native qualification passed, including model/protocol/tooling checks,
the host Antimatter save reference and native execution evidence. The
[Native Document Log](../../notebook/Development/2026-10-04%20Native%20Document%20Log.md) records the
current checkpoint and evidence. Use `make native-document-inspect`,
`make native-document-image` and `make native-document-smoke`.
Close/reopen preserves bytes only while the RAM service survives. Proposed
[ADR 0057](../adr/0057-prove-elf-loading-of-the-native-document-contract.md) implements
the same contract through the userspace ELF loader, with bounded qualification
complete. The [ELF Document Log](../../notebook/Development/2026-10-04%20ELF%20Document%20Log.md)
records the final evidence and unchanged native client bytes. Use
`make elf-document-inspect`, `make elf-document-image` and
`make elf-document-smoke`. Proposed
[ADR 0058](../adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md)
qualifies bounded native editor replacement with the service retained. All
50 qualification checks and two development-image restorations passed; the
[Native Relaunch Log](../../notebook/Development/2026-10-04%20Native%20Relaunch%20Log.md)
records the completed evidence.
Its oracle distinguishes discarded queued saves from committed saves with lost
replies, checks fresh authority and verifies exact recovered bytes. Host tests
passed: 491 for the relaunch kernel, 531 with all kernel features, 207 for tooling,
and 18 each for the editor and document protocol. TCG/Intel and KVM/AMD passed at
96 MiB and 320 MiB with matching captures. Use `make native-relaunch-inspect`,
`make native-relaunch-image` and `make native-relaunch-smoke`.

Proposed [ADR 0059](../adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md)
implements adjacent native/ELF replacement scenarios against the same known-byte
and semantic oracle. Loader ownership is reclaimed before any replacement run;
each initial and replacement owner uses the selected route image. All 52
qualification checks and two development-image restorations passed with 369
frozen source files unchanged. The strict 222-record proof passed TCG/Intel and
KVM/AMD at 96 MiB and 320 MiB with matching captures. Direct comparison also
matched all 99 lifecycle, surface and packet records per route to the qualified
native baseline in all six backend/size runs, excluding only the event prefix,
route label and individually bounded timer counts. The
[ELF Relaunch Log](../../notebook/Development/2026-10-04%20ELF%20Relaunch%20Log.md)
records the full evidence.
Use `make elf-relaunch-inspect`, `make elf-relaunch-image` and
`make elf-relaunch-smoke`. Scoped filesystem persistence with explicit durability
semantics follows that comparison.

## Completed native startup and private arena increment

Proposed [ADR 0055](../adr/0055-prove-native-startup-and-private-userspace-arenas.md) implements explicit native
startup data and owned userspace working memory. The opt-in four-region profile
adds one guarded RW/NX arena; default launch and circle policies remain strict
three-region profiles. Two retained tasks own 66 frames and twelve leases.
Descriptor fields describe the launch while live kernel bindings enforce
memory and channel authority. The same native client uses two documents and
layouts, with guard/NX fault cases followed by a fresh zeroed launch.

All 43 qualification checks and two image restorations passed; the
[Native Runtime Log](../../notebook/Development/2026-10-04%20Native%20Runtime%20Log.md) retains current receipts
and failures. Use `make native-runtime-inspect`, `make native-runtime-image`
and `make native-runtime-smoke`. ADR 0056 implements the bounded native document
follow-up above, now qualified; ADR 0057 also qualifies its bounded ELF
comparison. This remains
a Rust proof client, not full Antimatter/Electron.

## Completed shared two-caller userspace veil increment

The preceding proposed [ADR 0054](../adr/0054-prove-a-shared-multi-caller-userspace-veil.md) qualifies two independent Linux editor
producers and two native services sharing one supervised veil. Exact sealed
membership, per-caller routes and copied-memory isolation preserve the native
baseline. A caller fault leaves its peer progressing; a veil fault revokes both
dependents and discards its staged effect while both native services survive.
All 41 qualification checks and two image restorations passed, including TCG/Intel and
KVM/AMD at both guest sizes with capture comparison. All twelve preceding
embedded artifact blobs remain unchanged and no OOM occurred. The
[Shared Veil Log](../../notebook/Development/2026-10-04%20Shared%20Veil%20Log.md) records the frozen-source and compiled-code evidence.
This is a fixed two-member serial service, not full Antimatter/Electron or a
general process/runtime service.

## Preceding supervised userspace veil increment

Proposed [ADR 0053](../adr/0053-prove-a-supervised-userspace-syscall-veil.md) implements three retained
ring-3 tasks: the unchanged Linux producer, unchanged native service and a
native-entry veil. An 80-byte copied request carries raw registers and an opaque
request sequence. Linux syscall-number normalization, descriptor routing,
errno, readiness and return/yield/retire policy execute only in the veil.

The scheduler blocks the exact caller and selects its sealed veil while retaining
the service queue. Native mechanisms validate inactive caller-memory ownership,
stage one inert effect and preflight the reply before publishing it. Bad or stale
completion preserves the request and stage. A veil fault discards the stage,
revokes authority, contains the blocked caller and continues the native service.
The proof compares the seven editor states against the native reference and
adds queued-producer-fault and staged-veil-fault routes.

Qualification passed 39 matrix jobs and two restorations; the [Userspace Veil Log](../../notebook/Development/2026-10-04%20Userspace%20Veil%20Log.md)
records the exact image, stack, host-test and TCG/KVM evidence. This does not
rerun the userspace ELF loader: host-qualified artifacts and their manifest
supply the fixed tasks. Linux startup remains an explicit controller fixture.
ADR 0054 extends the fixed membership above. General runtime capabilities and
full Antimatter/Electron remain subsequent gates.

## Completed Linux startup and record-device checkpoint

Proposed [ADR 0052](../adr/0052-prove-linux-startup-and-record-device-syscalls.md)
adds an inspected Linux producer to the qualified native service comparison.
Its owned stack contains argc/argv/empty envp and only truthful AT_PAGESZ/AT_ENTRY
metadata. A separately owned entry session saves and restores EFER/STAR/LSTAR/
FMASK, selects supervisor storage before touching a stack and returns with
checked IRETQ. The native peer keeps SCE disabled.

Read/write/poll with timeout zero, sched_yield and exit operate over preopened
Anomalous record devices, not pipes. Input read copies and acknowledges atomically;
failed copies preserve queued data, and poll preflights its whole result array.
The controller authenticates exact task bindings; the packet payload remains
opaque. Normal and queued-UD2 routes match the native editor/service
semantics and final pixels while restoring every owner and entry control.

Qualification completed on 2026-10-04: all 37 matrix jobs and two restoration
jobs passed on 313 unchanged frozen source files. TCG and actual-host KVM passed
at 96/320 MiB, with identical kernel/image bytes and whole-frame captures.
The final matrix peaked at 2.310 GiB with all memory-event counters zero and no
host OOM kills. Evidence is in the
[Linux Application Log](../../notebook/Development/2026-10-04%20Linux%20Application%20Log.md).
The fixed personality adapter is feature-private proof-controller policy.
At that checkpoint, userspace policy placement was the next gate; ADR 0053
implements the bounded follow-up above. No stock libc, arbitrary Linux
executable, signals, TLS or Electron port is claimed.

## Completed userspace ELF loading increment

Proposed [ADR 0051](../adr/0051-prove-userspace-elf-loading-against-native.md)
keeps executable parsing in an independently executing loader vessel. The host
derives a compact deployment ELF while preserving the inspected client's exact
RX and initialized RW bytes. The safe `no_std` parser accepts only ELF64
little-endian ET_EXEC/EM_X86_64, entry `0x400000`, one RX load of at most 32 KiB
at that address, and one RW load of at most 8 KiB at `0x800000`. The file is at
most 64 KiB with two program headers, no section table, checked aligned spans,
zero gaps and no trailing bytes. This is an intentionally restricted subset.

Two loader roles exercise copied-plan admission, including wrong-role and
failed-copy requests, malformed plans, a geometrically valid plan that differs
from the trusted manifest, and duplicate offers. The 96-byte plan carries no
identity, grant or physical-address authority. Both loader tasks and their
resources retire before application allocation. The controller then runs the
same inspected client/service through flat and ELF-derived spans; initialized
RW readback, a user-read nonzero canary, BSS/page/stack zero-fill, service
counters, seven surface hashes, exact final pixels and cleanup must agree.
Timer counts may differ. Each route retains its own normal and queued-packet
fault scenarios and presentation; the native receipt describes the ELF route.

Qualification completed on 2026-10-04: all 35 matrix jobs and two image
restoration jobs passed with 302 frozen source files unchanged. TCG and
actual-host KVM passed at 96 MiB and 320 MiB plus separate captures, with
identical proof kernel/image bytes and exact matching editor pixels. Normal
and workspace images were restored unchanged from the preceding native
checkpoint. The
[ELF Application Log](../../notebook/Development/2026-10-03%20ELF%20Application%20Log.md)
records final evidence. ADR 0051 remains Proposed. This implementation supplies
bounded guest-side ELF loading with a native personality. It supplies neither
Linux startup/syscalls nor a full Antimatter/Electron runtime. ADR 0052 is the
separately qualified Linux comparison above.

## Preceding bounded native application increment

Proposed [ADR 0050](../adr/0050-prove-a-bounded-native-application-contract.md)
separates the Rust editor model, canonical wire codecs, opaque kernel channels
and independently executing surface/input service. Six scripted commands
produce seven surfaces matching real Antimatter selection replacement,
theme/selection preservation and undo. A producer may mutate its source buffer
after submission; the service owns the earlier copied snapshot. Failed copies,
wrong roles and closed channels must preserve the valid state and retry.

The service retains the final surface before acknowledgement, yields for the
producer's normal exit, then stages its independent copy. Both tasks retire
before the bootstrap-only framebuffer commit. A second scenario revokes a
queued surface on the producer's exact fault and preserves the earlier frame.
At this checkpoint the host inspected a fixed ELF build container and extracted
bytes. ADR 0051 adds runtime ELF admission separately; neither profile supplies
a foreign execution personality.

All 33 matrix jobs and two image-restoration jobs passed on 291 unchanged
frozen files. TCG/KVM passed both memory sizes and separate captures with
identical native images and public editor pixels. No OOM event occurred. The
[Native Application Log](../../notebook/Development/2026-10-03%20Native%20Application%20Log.md)
records the bounded scope and
[`native-application-reference.cjs`](../../scripts/native-application-reference.cjs)
checks the stored trace against the Linux checkout's actual CodeMirror and
production editor bridge. This host reference does not execute Antimatter
inside Anomalous. Physical input, files, full frontend parity, a broader Ny
runtime and stable application/executable contracts remain open.

## Preceding bounded Ny-object increment

Proposed
[ADR 0049](../adr/0049-qualify-a-bounded-freestanding-ny-object.md)
keeps the pinned compiler's scalar object intact and links only an inspected
startup shim. The host validates object imports/relocations, complete linked
extents, embedding metadata and final kernel bytes. The kernel receives an
immutable fixed payload, not an ELF parser or hosted runtime.

Two scenarios target the generated ABI-mismatch branch and an injected failure
after actual Ny computation, each followed by a successful peer. The source
checks integer/f64 globals and 50,000,000 iterations. Startup checks copied
initialized data, BSS zero-fill and System V stack alignment; actual timer
observations must land inside generated code. Checked copies bind messages and
results to the current owner before terminal cleanup and kernel CPU restoration.
All 30 targets passed in 1,293.38 seconds on 272 unchanged frozen files. The
full check passed 440 default, 449 Ny-feature and 483 all-feature kernel tests,
plus 119 xtask tests. Both RAM sizes passed under TCG and actual AMD-host KVM,
with identical proof images and exact restoration of 24,149/81,493 free frames.
Normal/workspace images were restored unchanged, and no OOM event occurred.
The [Ny Object Log](../../notebook/Development/2026-10-03%20Ny%20Object%20Log.md)
records final evidence and limits.
The task-bound surface/input prototype now follows under ADR 0050 above;
full frontend integration and workspace persistence remain open. General Ny
runtime/target support, allocation, TLS, external executable admission and a
stable service ABI remain open.

## Preceding internal application-profile increment

Proposed
[ADR 0048](../adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
adds a removable `application_profile_test` composition over the retained
execution owner. The [CPU session](../../kernel/src/arch/x86_64/application_cpu.rs)
uses eager FXSAVE64/FXRSTOR64 with clean handler state, admitted MXCSR bits,
exact original kernel-state/control restoration and verified image clearing.
AMD/Hygon admission requires `XSaveErPtr`; TLS, AVX/XSAVE and unowned fast syscall
entries are excluded. The [copy planner](../../kernel/src/arch/x86_64/user_copy.rs)
and retained mapping checks preflight the exact current owner and every byte's
permissions before any copy of up to 4096 bytes begins.

The [native runtime](../../kernel/src/arch/x86_64/application_profile_runtime.rs)
completed four task pairs, 256 preemptions and 320 timer ticks per VM, with
20 successful and 40 rejected copies. Selected #DE/#UD/#GP/#PF containment
accepts terminal user faults without requiring a returnable RIP/RSP, while
every surviving return still passes ordinary admission. The last current task
faults back to the retained bootstrap continuation before its storage is freed.
All 28 targets passed in 1,153.82 seconds of summed target time on 265 unchanged
frozen non-documentation files. The fresh full check passed 440 default,
483 all-feature and 449 application-feature kernel tests plus 106 xtask tests.
At 96 MiB and 320 MiB, TCG Nehalem (`GenuineIntel`) and actual-host KVM
(`AuthenticAMD`) each accepted 16 records and restored 24,146/81,490 free frames.
Only `cpu_vendor` differed among the eight feature events. Normal and workspace
images match the preceding checkpoint. The final matrix had no memory-limit
or OOM event; no new VirtualBox proof was run. The
[Application Profile Log](../../notebook/Development/2026-10-03%20Application%20Profile%20Log.md)
records final evidence, the corrected initial lint failure and resource measurements.
No complete untrusted-program profile, #MF/#XM recovery, stable ABI, native Ny
startup, or Antimatter execution follows from this fixture.

## Preceding owned sustained-dispatch increment

Proposed
[ADR 0047](../adr/0047-prove-sustained-dispatch-over-retained-launches.md) adds an
[aggregate execution owner](../../kernel/src/arch/x86_64/owned_execution.rs)
and [native runtime](../../kernel/src/arch/x86_64/owned_dispatcher_runtime.rs)
around two prepared launches. It retains 36 frames (18 data, 18 tables), ten
bound leases, two 16 KiB supervisor high-alias stacks, and activation/cleanup
progress. Source admission requires complete initialized linked-rodata bounds;
plans and trusted fixture identity remain separate from authority.

Narrow scheduler/task withdrawal handles only ready, queued initial contexts.
It rejects any global current task or reservation and inconsistent identity,
state, or attachment accounting before mutation, preserving survivor queue
order and sequence cursors. Native rollback uses a real one-slot scheduler to
reject the second admission, withdraw the first, and cancel both launches.
Two failed privilege-stack clears retain resources through cleanup retries.
Clear/release failure is retryable; failed mapping revalidation remains retained
fail-closed and is not recoverable merely by repeating `finish`.

The native users complete 64 real preemptions with independent progress and
intact canaries at nested RSP values of stack top minus 24 bytes. Task 0's
exact armed UD2 fault leaves task 1 running for 16 further ticks without
artificial redispatch, then task 1 exits. The faulted owner's mapping and stack
remain retained until both tasks are terminal. Bootstrap CR3, TSS.RSP0,
CR0.TS, and the kernel continuation are restored before full reclamation.

Six scheduler-withdrawal tests, four task-withdrawal tests, six strict verifier
groups, freestanding feature Clippy, and TCG/KVM runs at 96 MiB and 320 MiB
passed. Each native run accepted 15 records, including seven identical feature
records across backends, and restored 24,150/81,494 free frames. All 26 targets
qualified across documented phases in 1,491.42 seconds of selected target time.
The final host check passed 420 default, 463 all-feature, and 429 owned-feature
kernel tests plus 100 xtask tests. All 258 final frozen files remained unchanged
after a narrow display-capture harness correction. The
[Owned Sustained Dispatch Log](../../notebook/Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
records the interrupted attempt's diagnostic limit, final hashes and backend
comparison, and 4.002-GiB peak scope memory with no OOM under a 6-GiB cap.
`make owned-dispatcher-smoke` runs this separate proof at both RAM sizes.
That checkpoint selected the application CPU/copy/fault gate now qualified
within ADR 0048's stated limits above.
No general loader, stable ABI, FPU/TLS ownership, or native Antimatter execution
is supplied by the preceding ADR 0047 fixture.

## Preceding retained prepared-launch increment

Proposed
[ADR 0046](../adr/0046-prove-retained-prepared-launch-ownership.md) adds the
internal [prepared launch owner](../../kernel/src/arch/x86_64/prepared_launch.rs)
and [task cancellation transition](../../kernel/src/task.rs). Preparation
rechecks the admitted entry against the exact three-region GPR-only mapping
profile, builds and populates inactive mappings, verifies the payload and tree,
and retains circle/address-space/mapping attachments. It creates no task.
Explicit commit checks those attachments and publishes the ready task as its
final fallible operation; fixture identity metadata remains separate from the
inert load plan.

Cancellation accepts partial preparation, an unstarted ready task, or a terminal
task. `cancel_unstarted` makes a ready task with no attached context terminal
before mappings can be cleared. Running tasks and unexpected attachments are
rejected. Each completed release/detachment is retained before the next step,
so mapping-clear failures can be retried without a runnable task or lost lease.
Existing task/circle tombstone-clearing failures remain terminal and poison the
owner. Root restoration and continuation withdrawal remain the separate
activation session's obligation before cleanup.

The native fixture exercises thirteen failure/cancellation cases: four
allocation-clear failures, one table-write failure, one payload failure, one
circle-capacity failure, one prepared-owner cancellation, and five failed
cleanup/retry cases after committing unstarted tasks. It then prepares and
commits the original two real users through the same owner: B faults after its
reports, A continues and exits, and complete ownership is reclaimed. The strict
transcript adds `kernel.memory_test.launch.verified` while preserving preceding
evidence.

Four focused task-transition tests, eight owner tests, seven memory-verifier
tests, freestanding Clippy, and memory proofs at 96 MiB and 320 MiB under TCG/KVM
passed. All 24 regression targets passed in 1,059.31 seconds of combined target
time with 254 frozen non-documentation files unchanged. Both backends restore
24,148/81,492 free frames; normal and workspace images match recovery. The
[Prepared Launch Ownership Log](../../notebook/Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records the run evidence.

ADR 0047 subsequently integrated retained launch ownership with bounded sustained
dispatch, and ADR 0048 added the internal CPU/copy/fault profile. ADRs 0049 and
0050 then qualified the bounded Ny object/startup and Rust application/service
compositions above. Broader executable admission, language runtime and full
Antimatter integration remain open; this earlier trusted fixture establishes
none of them. Permanent layout, TLS, additional CPU-state ownership and global
W^X remain separately gated.

## Previous owned-user-execution increment

Proposed [ADR 0044](../adr/0044-prove-owned-low-address-user-execution.md) adds a
pure [entry plan](../../kernel/src/arch/x86_64/user_entry_plan.rs), exact
[user exception predicates](../../kernel/src/arch/x86_64/user_fault_gate.rs),
and a [native proof](../../kernel/src/arch/x86_64/user_mapping_test.rs).
Two real task/circle identities retain separate three-region mixed roots: RX
code at 4 MiB, two private RW+NX data pages, and two guarded RW+NX stack pages.
The complete fixture owns ten data frames, eighteen table frames, and eight
leases. Code admission ends at the declared initialized byte count; it does not
admit the rest of the mapped page. Initial and resumed stack alignment are
checked separately, with noninitial stack pointers admitted for normal calls.

The root sequence is bootstrap → B → A → bootstrap. B's two exact UD2 reports
resume at different noninitial stack pointers, the first inside a nested call;
its later read of supervisor kernel text faults with exact error 5. A then runs
two reports and exits, proving continued peer progress and private-data
separation. The private feature handshake has no syscall-ABI status. The retained
activation session outlives user execution, and bootstrap restoration/root
withdrawal precede owner cleanup, child detachment, and identity destruction.

The shared exception snapshot now reads actual hardware-saved RSP and SS from
the complete 176-byte long-mode frame, including ordinary CPL0 and IST entry.
Feature predicates additionally check frame residency, exact root, selectors,
flags, instruction, saved stack, and the appropriate error/CR2. The privilege
stack is wiped after each terminal return, its TSS selection preserved, TS
restored, and saved continuations cleared.

All 24 validation targets passed in 1,346.9 seconds of combined target time,
with 240 frozen source/configuration files unchanged. TCG and KVM memory proofs
at 96 MiB and 320 MiB each produced 23 structured records, 20 selected lifecycle
records, and exact restoration of 24,150/81,494 free frames. Focused checks passed
107 x86 tests, eight memory-transcript tests, and freestanding Clippy; full suites
passed 398 default kernel, 441 all-feature kernel, and 92 xtask tests. The
[Owned User Execution Log](../../notebook/Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records matching initial/TCG/KVM memory images. Normal, dispatcher, and workspace
image hashes changed with the shared diagnostic correction; direct feature
exclusion and all runtime regressions passed.

That checkpoint selected retained preparation, explicit commit, and retryable
cancellation as its follow-up. ADR 0046 implemented that bounded ownership
composition; ADR 0047 added sustained scheduling, followed by the internal
ADR 0048 application profile and its completed scoped validation recorded above.

## Previous resident-activation increment

Proposed [ADR 0043](../adr/0043-prove-resident-mapping-activation.md) adds
[checked resident geometry](../../kernel/src/arch/x86_64/resident_plan.rs),
[exact fault probes](../../kernel/src/arch/x86_64/resident_probe.rs), and a
[separate activation session/native proof](../../kernel/src/arch/x86_64/active_mapping_test.rs).
Each root owns ten table frames and seven data frames. Explicit supervisor
4 KiB leaves map kernel text RX, constants R+NX, and data/BSS plus the complete
Threshold stack RW+NX. All other pages in the reserved low island are absent.
Admission checks actual GDTR, IDTR, TR, TSS/IST state and resident stack bounds.

A retained session borrows both stages and the shared physical-access owner
through A → B → A → bootstrap. The low user code address and a separate physical
probe both equal 4 MiB; reading the physical marker through the high alias
proves it is independent of the active low virtual mapping. Six exact CPL0
faults test read-only stores, guard reads, and NX fetches. Their single-use
predicates check root, RIP, CR2, error, selectors, flags and the complete saved
stack frame before changing only the fixed continuation RIP. SMAP, SMEP and
other unsupported CPU controls are excluded. Post-hardware validation permits
only A and, on writable 4 KiB leaves, D. Bootstrap restoration and borrower
withdrawal precede clearing and releasing every owned frame.

See the [Resident Mapping Activation Log](../../notebook/Development/2026-09-26%20Resident%20Mapping%20Activation%20Log.md)
for validation evidence: 94 focused x86 tests, eight strict memory-transcript
tests, freestanding Clippy, and all 24 regression targets passed. Native memory
proofs at 96 MiB and 320 MiB under TCG and KVM each verified twenty exact records
and restored all resources. All 236 frozen source files remained unchanged;
normal, dispatcher, and workspace images match the preceding checkpoint.

The separate owned-user increment above now exercises bounded ring-3 entry,
resumed stacks, and terminal task ownership using this residency. The CPL0
probes remain distinct evidence and establish neither general fault recovery
nor production user-copy. At the resident checkpoint they read hardware SS:RSP
independently; ADR 0044 now corrects the shared snapshot contract as well.
Native launch and compatibility adapters remain later steps.

## Previous owned-construction increment

Proposed [ADR 0042](../adr/0042-prove-inactive-owned-user-mapping-construction.md)
implements [retained staged ownership](../../kernel/src/arch/x86_64/owned_mapping.rs),
[temporary privileged physical access](../../kernel/src/arch/x86_64/physical_access.rs),
and a [native construction proof](../../kernel/src/arch/x86_64/owned_mapping_test.rs).
Four regions use seven data frames, nine table frames, and five noncopyable leases.
The proof checks every mapping and guard, injects five allocation-clear failures,
one population-write failure, and six cleanup-clear failures, then verifies exact
resource restoration and high-alias removal. The original root remains selected.
Host tests additionally cover all population writes and every readback position
of a four-table fixture. Focused checks and all 24 regression targets passed,
including the 96-MiB/320-MiB memory proof under both TCG and KVM. Evidence is in the
[Owned Mapping Construction Log](../../notebook/Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md).

This closes a bounded inactive construction/cleanup proof. The resident increment
above adds a separate admission and activation boundary. Original user-only roots
remain ineligible for selection; their readiness flag confers no activation
authority. The retained physical alias still prevents a global W^X claim.

## Previous memory-planning increment

Proposed [ADR 0041](../adr/0041-plan-bounded-user-mappings-and-physical-access.md)
implements inert [user mapping plans](../../kernel/src/arch/x86_64/mapping_plan.rs)
and [physical-window geometry](../../kernel/src/arch/x86_64/physical_window.rs).
The four-region host example includes code at 4 MiB, read-only data crossing a
2-MiB table boundary, separate writable data, and a distant guarded stack. Its
seven data pages need nine user-tree table frames. Independent page enumeration
checks the branch counts. These are planned addresses, not active translations;
the existing bootstrap PML4-zero reservation still applies to the live mapper.
Focused checks and all 23 regression targets passed; the normal, dispatcher,
and workspace images remain byte-identical to the preceding checkpoint. See the
[User Mapping Plan Log](../../notebook/Development/2026-09-26%20User%20Mapping%20Plan%20Log.md).

ADR 0042 consumes these plans for owned inactive construction. Planning itself
remains side-effect-free and proves neither allocation cleanup nor activation.

## Previous sustained-dispatch checkpoint

Proposed [ADR 0039](../adr/0039-prove-bounded-sustained-native-dispatch.md) selects
the sustained-dispatch proof. It introduces no artifact format or foreign
personality and leaves the existing desktop available for development. See the
[Sustained Native Dispatch Log](../../notebook/Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md)
for implementation and validation evidence. All 23 validation targets passed,
including final TCG/KVM proofs and retained desktop regressions. A separate
VirtualBox attempt was blocked by host KVM ownership of AMD-V and its isolated
VM was cleaned up. These results close this bounded proof, not the remaining
execution prerequisites above.
