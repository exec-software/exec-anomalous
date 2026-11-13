---
title: "Anomalous — Current State"
date: 2026-11-13
tags: [status, milestone, boot, kernel, memory, interrupts, input, shell, session, display, desktop, privacy, waiting, circle, ring3, scheduler, supervision, rites, x86-64]
status: active
domain: foundations
---

# Anomalous — Current State

## Current host checkpoint — immutable typed arrays

[ADR 0109](../../docs/adr/0109-add-bounded-immutable-typed-arrays.md)
adds explicitly selected profile 5 for homogeneous integer/text arrays, lengths
and indexed reads. Private handles preserve aliases and helper returns, including
computed text elements. Fixed storage allows 128 descriptors and 1,024 cumulative
elements per run, with at most 256 per literal. Out-of-range reads follow the
pinned default semantics: zero or empty text. Every execution boundary clears
all array and text storage. Earlier profiles and production intent requirements
retain their contracts.

The new 87-case proof records 36 exact semantic matches, one matching fault,
seven finite policy contrasts and 43 validation refusals. Eight of 45 unchanged
raw corpus invocations match, including five of 38 roots; the complete
`control-flow.matrix` edition is newly supported. The remaining 37 refusals are
unmet coverage, and all 45 still refuse production admission. The
[Typed Arrays Log](../Development/2026-11-13%20Typed%20Arrays%20Log.md) records
qualification and remaining gates. This adds no native NIL execution or Linux
compatibility behavior. All 35 final qualification jobs pass on 692 unchanged
source files; independent audits confirm all six proof suites, native artifact
continuity and original-checkout preservation.

## Previous host checkpoint — computed text and hexadecimal integers

[ADR 0108](../../docs/adr/0108-add-bounded-computed-text-and-hexadecimal-integers.md)
adds explicitly selected profile 4. Text/text concatenation creates immutable
values in a bounded decoded arena that survives helper returns and retains
aliases. Literals decode exactly once; every execution boundary clears the
arena. Signed-i64 hexadecimal constants follow the pinned reference, including
negative minimum and explicit overflow refusal. Earlier profiles retain their
accepted grammar and production intent remains required.

The new 55-case proof covers semantic matches, matching faults, finite resource
contrasts and validation refusals. Seven of 45 unchanged raw corpus invocations
match, including four of 38 roots. The remaining 38 are unmet coverage; all 45
still refuse production admission. The [Computed Text Log](../Development/2026-11-13%20Computed%20Text%20Log.md)
records qualification and remaining gates. This adds no native NIL execution or
Linux compatibility behavior. All 32 final qualification jobs pass on 685
unchanged source files; independent audits verify the five proof suites, native
artifact continuity and original-checkout preservation.

## Previous host checkpoint — separate conformance and admission

[ADR 0107](../../docs/adr/0107-separate-host-conformance-from-production-admission.md)
separates raw language experiments from production manifest admission. The
opt-in host feature validates unchanged editions into a distinct checked witness;
missing main intent is represented as absence. Existing present-annotation,
whole-file authority, CFG and resource checks remain mandatory. Production
packaging and dispatch retain their admitted witness and required authored
intent. The raw feature refuses freestanding compilation.

The new proof passes 27 boundary cases. Four of 45 unchanged raw corpus
invocations match reference output and exit status: one of 38 root cases plus
three auxiliary executions of the same second edition. The remaining 41 remain
unmet coverage. Production independently refuses all 45 with the prior policy
outcomes; the existing 126-, 77- and 61-case proofs pass unchanged.

The [Host Conformance Boundary Log](../Development/2026-11-13%20Host%20Conformance%20Boundary%20Log.md)
records separate raw and production corpus ledgers. No native NIL execution,
new kernel feature or Linux service advancement is claimed. Full unmodified
corpus qualification and native integration remain separate gates.
All 29 final qualification jobs pass on 678 unchanged source files; independent
audits confirm the evidence and original checkout preservation.

## Previous host checkpoint — compiler scalar control flow

Accepted [ADR 0106](../../docs/adr/0106-admit-bounded-compiler-scalar-control-flow.md)
adds explicit host-only profile 3 while preserving profile 2's accepted grammar
and refusals. The new route shares the validator and executor, admitting only
local operands with the bounded `$ny.temp.<ASCII_identifier>.<decimal>` spelling.
The counter has canonical decimal spelling, and the complete name fits 128
bytes. It also normalizes bare `ADD` to the existing `i64` operation, with
integer operands proved on reachable paths. Other dollar-name positions,
dotted temporary prefixes and other untyped arithmetic remain refused.

The same fixed storage, whole-artifact authority union, stack/local validation
and runtime limits apply. No native feature, mapping profile or edition
execution is added.

All 27 final jobs passed on their first attempts; the independent audit confirms
672 unchanged source files. The profile-3 proof passes 61 cases: 27 exact
semantic matches, one fuel-policy refusal against a completed finite reference
run and 33 admission refusals. Seven complete compiler outputs match exactly;
the eighth supplies the finite-reference fuel refusal. Another 28 checks
preserve profile-2 raw-syntax refusals. The existing 126-case profile-1 and
77-case profile-2 proofs pass
unchanged; each profile still refuses all 45 unmodified corpus invocations.

Fresh TCG native admission, native delivery and Linux regressions pass at
96/320 MiB. Five retained native admission artifacts remain byte-identical to
the prior KVM-qualified checkpoint; no fresh KVM run is claimed. Linux still
stops at unanswered sequence-12 `mmap` after eleven returns. The
[Compiler Scalar Flow Log](../Development/2026-11-12%20Compiler%20Scalar%20Flow%20Log.md)
records the final evidence separately from the accepted profile-2 checkpoint
below. The full unmodified corpus and native execution gates remain closed;
missing authored intent is not supplied or waived.

## Previous host checkpoint — bounded functions and control flow

Accepted [ADR 0105](../../docs/adr/0105-validate-bounded-function-control-flow-before-host-execution.md)
adds a separate host-only profile-2 admission and execution path. It validates
static function calls, integer branches, loops, typed stack joins and definite
local initialization before execution. Complete syntax and reference checks,
plus the authority union, include dead instructions and unused functions.
Caller-owned workspaces bound validation and iterative execution; recursion
uses explicit frames. Profile 1 and the native admission path retain their
existing contracts. No native vessel links this executor.

All 25 final qualification jobs passed on their first attempts; the independent
audit confirms 651 unchanged source files. The proof passes 77 cases: 38 exact
semantic comparisons, two exact traps, four finite policy contrasts, four
candidate bounded cases and 29 admission refusals. It also checks eighteen
profile-1 raw-syntax refusals.
All 126 profile-1 differential cases pass unchanged, alongside 22 flow and
31 executor unit tests.
All 97 compiler calls and 45 interpreter calls covering the reference suite's
38 root fixtures are captured. All 45 candidate corpus attempts remain
admission refusals: neither full interpreter qualification nor native execution
is established.

Fresh TCG native admission, native delivery and Linux regressions pass at
96/320 MiB. Five retained native admission artifacts are byte-identical to the
previous KVM-qualified checkpoint; KVM was not rerun in this increment. Linux
still stops at its unanswered sequence-12 `mmap` after eleven returns. The
[Program Flow Executor Log](../Development/2026-11-11%20Program%20Flow%20Executor%20Log.md)
records the final evidence, retained development attempts and remaining gates.

## Previous host checkpoint — bounded straight-line NIL execution

Accepted [ADR 0104](../../docs/adr/0104-develop-a-bounded-host-executor-behind-the-corpus-gate.md) adds an experimental `no_std`, unsafe-free executor
for the admitted profile-1 NIL subset. It consumes the immutable admission
witness, runs `main`, preserves full-width integer results and mediates every
console request through explicit identity, authority and output limits. It is
host-only; no native vessel links the executor.

All 24 final qualification jobs pass on 631 unchanged source files. The rebuilt
native admission loader also passes TCG/KVM regressions, with 18,666 RX bytes
and a 13,440-byte conservative stack estimate inside its existing profile.
The bounded differential proof passes 126 cases: 100 exact semantic matches,
20 matching arithmetic refusals and six explicit output-policy refusals. The
three complete compiler fixtures return 42, 0 and 79, with exact console bytes.
The unchanged reference suite passes all 38 root fixtures and its auxiliary
checks. Every one of its 45 interpreter invocations is retained and attempted
by the candidate; all remain admission refusals, not compatibility passes.

This checkpoint did not qualify function or control-flow semantics. The newer
profile-2 work above addresses a bounded subset; full corpus qualification,
aggregates, faults and wider service families remain open. Native effect
mediation and execution are also unqualified. The
[Host Program Executor Log](../Development/2026-11-11%20Host%20Program%20Executor%20Log.md) records the bounded evidence, reproduction
commands and remaining gates. Linux remains at its unanswered sequence-12
`mmap` boundary.

## Current native checkpoint — compiler-produced manifest admission

Accepted [ADR 0103](../../docs/adr/0103-admit-compiler-produced-manifests-in-a-native-loader.md)
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
NIL. No NIL edition executes in that native loader; interpreter corpus qualification
and mediated execution remain separate. The
[Native Manifest Loader Log](../Development/2026-11-11%20Native%20Manifest%20Loader%20Log.md)
records artifact identities, limitations and retained attempts.

## Previous host checkpoint — bounded program-manifest admission

Accepted [ADR 0102](../../docs/adr/0102-admit-bounded-program-semantic-manifests.md)
adds a shared `no_std` parser, unsigned `.re` envelope and host pack/inspect
commands. Admission rederives exact edition identity, plain authored intent and
console authority from a fully consumed bounded NIL subset, then checks an
explicit workload ceiling. The returned witness borrows the checked bytes;
it grants no native authority and executes no edition.

All 25 final jobs passed on their first attempts against 608 unchanged source
files, including eighteen manifest tests, nine focused tooling tests, 1,659
mutation refusals and twenty CLI cases. Fresh TCG regressions preserve the
native edition-delivery image byte for byte from qualified baseline `914d942`
and leave Linux at its unanswered sequence-12 boundary. The earlier TCG/KVM
native qualification remains separate below. The
[Program Manifest Admission Log](../Development/2026-11-11%20Program%20Manifest%20Admission%20Log.md)
retains this increment's evidence and remaining gates.

Positive fixtures at this host-only checkpoint were hand-authored NIL. Complete
default compiler outputs retain an external `nilaix` registration helper and
remain refused without rewriting. ADR 0103 above now supplies the bounded
compiler bridge and native admission; interpreter corpus qualification remains
next. This host increment advances neither native edition execution nor
the Linux sequence-12 boundary.

## Qualified native ABI baseline — bounded edition reads

Accepted [ADR 0101](../../docs/adr/0101-deliver-bounded-native-program-edition-bytes.md)
connects the ANPG edition grant to one separately retained immutable source.
The experimental service validates live task authority and the complete arena
destination before each bounded read, then returns the exact committed length
and sequence. EOF returns zero bytes and consumes a sequence; rejected requests
leave the destination and sequence unchanged. Scratch clearing, authority
revocation and source cleanup are part of the retained lifecycle.

The measured 5,605-byte client requires two RX pages in the named program
profile, with its existing 64 KiB arena. The native application's eight-page
code and 32 KiB arena are unchanged. Preparation releases allocator borrows
before guest transfer; teardown validates the resumed continuation and
reacquires the allocator. An authenticated service probe checks ordinary
reacquisition and nested-borrow denial without modifying the caller frame.

All 30 final jobs passed on 598 unchanged source files. The same reviewed image
passed at 96 MiB and 320 MiB under TCG/Nehalem and AMD KVM/host. Each successful
boot verified seven edition reads, including two EOF replies, 8,203 delivered
bytes including repeated ranges, seventeen rejected reads and complete cleanup.
Three provisioning refusals per backend also restored all resources. Exact
host counts, artifact identities and preliminary attempts are recorded in the
[Edition Read Log](../Development/2026-11-11%20Native%20Program%20Edition%20Read%20Log.md).
The fixture delivers synthetic binary bytes and executes no edition.
Native semantic-manifest integration and the selected Rust interpreter's corpus
qualification remain separate gates; the host admission increment is above.
The Linux regression still stops at the
unanswered sequence-12 request after eleven returns:
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`. Main entry and Linux exit
remain unproved.

## Previous native ABI checkpoint — program console transport

Proposed [ADR 0100](../../docs/adr/0100-qualify-native-program-console-transport.md)
connects the ANPG console grant to a service bound to the live task, with checked
owned-memory copying, whole UTF-8 validation and committed sequence/length
replies. The fixture verifies two writes totaling 60 bytes, sixteen rejected
requests, and complete cleanup across two arena bases. All 27 final jobs passed
on 503 unchanged source files, including 96/320 MiB runs under TCG/Nehalem and
AMD KVM/host with the same image. Exact results and failed attempts are in the
[Native Program Console Log](../Development/2026-11-11%20Native%20Program%20Console%20Log.md).
That console-only proof maps or executes no edition. Bounded edition delivery
is qualified above; native semantic-manifest integration and the selected Rust
interpreter's corpus qualification remain next.
Linux's live source-backed RX replacement remains a separate execution gate.

## Selected next major slice — existing Linux Antimatter through the Linux veil

On 2026-10-07, after trying the native reference, the user clarified that the
application target is the existing Linux Antimatter build, shipped in its
current Electron-based form. A native frontend or `.re` port is not expected.
Accepted [ADR 0073](../../docs/adr/0073-run-linux-antimatter-through-the-linux-veil.md)
resolves this direction. Product Milestone 1 is the actual application's launch,
rendering and input inside Anomalous through its Linux compatibility backend.
Native proofs remain the resource/service baseline. The next target is a small
unmodified dynamically linked Linux executable, its interpreter and libc, before
the larger process/file/display environment needed by Electron. The existing
narrow ELF and five-syscall record-device proofs do not run general Linux apps.
The dated native milestones below remain historical prerequisites.

The first Linux artifact slice now implements host-only multi-segment/PIE image
planning and failure-atomic initial-stack construction under
[ADR 0074](../../docs/adr/0074-plan-linux-images-and-initial-stacks.md). Actual
`true`, interpreter, libc and Electron layouts pass independent structural
inspection. That host-only checkpoint did not execute a Linux image. The owned
mapping and source prerequisites below now support the separate interpreter-entry
increment under ADR 0079; general Linux loader services remain open. The
[Linux Image and Startup Log](../Development/2026-10-07%20Linux%20Image%20and%20Startup%20Log.md)
records exact qualification and memory evidence.

The native multi-region prerequisite is implemented under
[ADR 0075](../../docs/adr/0075-prove-owned-multi-region-launch.md): nine owned
regions per task, exact initialized executable intervals, high stacks and
complete retained teardown. Two synthetic tasks use the inspected main/loader
geometry; they do not execute Linux binaries. The
[Owned Multi-region Launch Log](../Development/2026-10-07%20Owned%20Multi-region%20Launch%20Log.md)
records physical qualification and the stack/timer issues found during testing.
Copied loader-source admission is qualified under
[ADR 0076](../../docs/adr/0076-prove-owned-loader-sources.md). Sealed source owners outlive complete producer retirement and supply
fresh consumers without retaining uploader pointers. The
[Owned Loader Sources Log](../Development/2026-10-07%20Owned%20Loader%20Sources%20Log.md)
records the TCG/KVM/VirtualBox proofs, all sixteen required checks and affected
regressions, including corrected lint/stack/format failures and a guarded
memory-pressure abort before the successful retry. Operation budgets introduce no global
path limits.

Native task-owned FS bases are now qualified under
[ADR 0077](../../docs/adr/0077-prove-task-owned-fs-bases.md). Two tasks preserve
actual FS-relative data through native calls, 64 preemptions, a contained fault
and sixteen survivor callbacks, then explicitly clear TLS ownership before
mapping release. TCG/KVM at 96/320 MiB, VirtualBox at 320 MiB, all sixteen
required checks and ten affected regressions passed. The
[Task-owned FS Bases Log](../Development/2026-10-07%20Task-owned%20FS%20Bases%20Log.md)
records the 96-byte owner, 72-byte entry capsule, corrected storage-clearing
finding, emitted stack review and memory guard evidence.

The bounded Linux FS/eager-CPU composition is now qualified under
[ADR 0078](../../docs/adr/0078-compose-linux-fs-and-eager-cpu-ownership.md).
A Linux caller, native service and userspace veil retain separate per-task CPU
state under the existing eager owner. Userspace `ARCH_SET_FS`/`ARCH_GET_FS`
policy uses a sealed pending-caller effect; exact reply commit changes the
caller's retained FS value. The fixture verifies FS-relative memory, selected
caller x87/XMM15/MXCSR state, timer entry/return, cooperative switches, contained
faults and explicit retirement clearing. Earlier zero-TLS profiles retain
their requirements.

Final TCG/KVM boots at 96/320 MiB and the bounded capture oracle passed, together
with all sixteen required checks and twelve affected regressions. VirtualBox
booted but refused `application_cpu_amd_error_pointers` before userspace; this
is a capability refusal, not a guest execution pass. The
[Linux FS and Eager CPU Log](../Development/2026-10-07%20Linux%20FS%20and%20Eager%20CPU%20Log.md)
records exact artifacts, failed attempts, emitted reviews and memory evidence.

The previous one-boundary loader-entry checkpoint is recorded in
[ADR 0079](../../docs/adr/0079-reach-an-unchanged-linux-interpreter-service-boundary.md).
It mapped unchanged `true` and its interpreter from a separately provisioned,
retained source snapshot and entered the interpreter with a Linux initial stack.
The kernel admits native mapping/source facts; ELF interpretation and Linux
syscall policy remain outside it.

TCG and KVM boots at 96/320 MiB reached actual interpreter syscall 12, `brk(0)`,
at `0x600284c9` with return RIP `0x600284cb`. A separate native userspace veil
selected `-ENOSYS` retirement. All three tasks retired, source and mapping
resources were reclaimed, and TLS/CPU state was cleared or restored as required.
Linux never received that result: the caller was
retired without a return, and neither a Linux exit nor the main `true` entry was
observed. This is real interpreter execution to a service boundary, not `true`
completion. TCG/Intel and KVM/AMD used identical retained kernel/image bytes.
Provisioning tests rejected missing input, wrong length and wrong hash.
VirtualBox booted but refused `application_cpu_amd_error_pointers`, and cleanup
powered it off; no VirtualBox execution pass is claimed. All 29 required/affected
targets and an additional fresh CPU-workspace regression passed. The final
evidence audit passed, including independently reviewed baseline changes and
memory records, with no observed OOM. The
[Linux Loader Entry Log](../Development/2026-10-08%20Linux%20Loader%20Entry%20Log.md)
records exact artifacts, failed attempts, qualification and remaining limits.

The preceding increment advanced that same default-off profile under
[ADR 0080](../../docs/adr/0080-resume-an-unchanged-linux-interpreter-after-a-userspace-reply.md).
TCG and KVM at 96/320 MiB passed with identical image bytes: the userspace veil
answered `brk(0)` with the declared current break, `0x40009000`, and the
unchanged interpreter actually resumed. The saved before/after frames differ
only in RAX. Its next real boundary was syscall 21, `access`, at `0x60029489`:
mode 4, pointer `0x60033720`, naming `/etc/ld.so.preload` in initialized
read-only interpreter bytes.

The veil selected unsupported retirement at that second boundary without
returning its `-ENOSYS` bits to Linux. The exact thirteen-record proof covers
two raw calls, one caller return, seven native calls, three retirements and
complete ownership restoration. All three negative provisioning cases passed.
VirtualBox booted but refused `application_cpu_amd_error_pointers` before
userspace and was powered off. All 29 regression targets and the final evidence
audit passed. All five baseline images remained byte-identical, all 481 frozen
source files remained unchanged and no OOM was observed. The
[Linux Loader Reply Log](../Development/2026-10-08%20Linux%20Loader%20Reply%20Log.md)
records final results, retained evidence and the reviewed audit-label correction.

The preceding increment under
[ADR 0081](../../docs/adr/0081-answer-an-unchanged-loader-file-probe-in-userspace.md)
qualifies the preload-file reply. The userspace veil copies the actual caller
pathname through existing native operation 18 and returns `-ENOENT` from an
explicit declaration that this exact name is absent. Six rejected copy probes
and nineteen successful byte reads precede that second RAX-only reply. The
unchanged interpreter then reaches
`openat(AT_FDCWD, "/etc/ld.so.cache", O_RDONLY | O_CLOEXEC, 0)`. The third
pathname is reconstructed from retained source; this proof does not copy it
in the guest or return a third result to Linux.

TCG/KVM at 96/320 MiB, malformed source provisioning, all 29 regression targets
and the final evidence audit passed. Each fifteen-record transcript proves two
real returns, three raw calls, 34 native calls, 25 copy attempts (nineteen
successes and six rejections), cleared scratch and complete three-task resource
restoration. All five baseline images remained byte-identical, all 482 frozen
source files remained unchanged and no OOM was observed. VirtualBox again
refused the existing CPU capability gate and powered off. The
[Linux Loader Access Log](../Development/2026-10-08%20Linux%20Loader%20Access%20Log.md)
records final evidence, memory observations and exact scope.

The preceding increment under
[ADR 0082](../../docs/adr/0082-answer-the-loader-cache-probe-with-fresh-caller-authority.md)
copies the third pathname under fresh sealed caller authority and answers the
explicitly declared cache absence with `-ENOENT`. Separate reply journals and
verified clearing prevent cross-request evidence reuse. All four TCG/KVM
96/320 MiB runs pass three real returns, four raw calls, 59 native calls and
48 copy attempts (36 successes and twelve rejections), then complete three-task
retirement and resource restoration. All 29 regression targets passed after
repairing the presentation capture waiter's incomplete-record race; the first
failed attempt remains retained without a conclusive cause attribution.
Negative provisioning and the final evidence audit passed. All five baseline
images remained identical, all 482 final frozen source files were unchanged
and no OOM was observed. VirtualBox refused the existing CPU capability gate
and powered off without a semantic pass.

The fourth actual request was another `openat`, with pathname pointer
`0x7fff0000eec0` in writable stack memory. Its pathname remained unknown in that
proof, which did not copy it; initial source bytes cannot reconstruct runtime writes.
The
[Linux Loader Cache Log](../Development/2026-10-08%20Linux%20Loader%20Cache%20Log.md)
records the exact boundaries, qualification and next-service handoff.

The latest qualified guest increment under
[ADR 0083](../../docs/adr/0083-observe-the-loaders-live-pathname-before-file-service.md)
qualifies the live stack copy. All four TCG/KVM 96/320 MiB runs observed
`/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libc.so.6`, 55 bytes including
NUL, with equal complete terminal frames. Each run retains three actual Linux
returns, performs eighteen rejected copy probes and retires all three owners
with complete resource restoration. It does not answer the fourth request.
All 29 required/affected targets, three provisioning rejection cases and the
final evidence audit passed. All five development images remained identical
and all 482 frozen source hashes were unchanged. Peak guarded scope memory was
3.50 GiB, with no observed OOM or pressure abort. VirtualBox again refused the
existing CPU capability gate before userspace and powered off. The
[Linux Loader Live Path Log](../Development/2026-10-11%20Linux%20Loader%20Live%20Path%20Log.md)
records the exact evidence and preserved focused lint failure.

The qualified host checkpoint under accepted
[ADR 0084](../../docs/adr/0084-model-read-only-userspace-file-services.md)
qualifies an opt-in portable read-only userspace file service on the host. An explicitly complete namespace scope supplies exact byte-name
bindings and distinguishes absence from unsupported or out-of-scope requests.
Task-incarnation descriptor tables retain immutable object/version bytes across
same-name replacement in another view. Sequential reads use an explicit byte
budget, with independent offsets, short reads, zero-length reads and EOF.
Publication rejection preserves committed state; unknown publication or callback
unwinding revokes the service. Internal handle generations protect retained
handles, without claiming that a reused Linux integer descriptor rejects stale
intent. Name slices introduce no fixed path length or depth buffer.

The host qualification read the entire held libc file through two independent
descriptor tables, with the base library name present and the observed hwcaps candidate
absent in its configured test view. No such namespace decision is installed in
the guest: the existing v3 policy still declares only preload/cache absence.
The three-reply guest baseline remains unchanged, with the fourth request
captured and retired without another Linux return or a successful file open.

Both retained output streams equal all 2,190,608 pinned libc bytes, and the
independent host verifier checked all 2,739 read publications. All 29 regression
commands, fresh TCG/KVM 96/320 MiB proofs and the final evidence audit passed;
five baseline images remained identical. The unchanged memory guard stopped
an earlier pressure spike, after which a complete retry passed. No OOM was
observed; VirtualBox's existing CPU capability refusal remains unqualified.
The [Read-only File Service Log](../Development/2026-10-11%20Read-only%20File%20Service%20Log.md)
retains the exact receipts, failed attempts and native integration gaps.

[ADR 0085](../../docs/adr/0085-native-retained-file-source-reads.md) adds a
separate retained libc source and native bounded source reads. The native veil
consumes all 2,190,608 bytes in 536 reads, exercises 10 rejected reads, and reports
its consumed length/hash while the fourth Linux invocation remains suspended.
The existing process plan, startup source and ANLPOL03 policy remain unchanged;
libc adds 535 owned source pages and one lease, with no additional Linux mappings.
The [Native File Source Log](../Development/2026-10-11%20Native%20File%20Source%20Log.md)
tracks the qualification receipts and remaining integration work.

TCG/KVM passed at 96/320 MiB, all 29 required/affected commands passed, and six
negative provisioning cases refused before userspace entry. Four baseline images
are byte-identical; the editor image has only independently reviewed diagnostic
line/checksum changes. An earlier pressure abort was preserved and retried under
the unchanged guard, with no observed OOM kill. VirtualBox still refuses its
existing CPU capability gate before source import.

The portable file-service core now supports a statically dispatched immutable
source with bounded per-invocation scratch under
[ADR 0086](../../docs/adr/0086-read-immutable-file-sources-in-bounded-chunks.md).
It retains the slice-backed API and a single descriptor/offset state machine.
Source failures and incorrect returned lengths publish no bytes or offset
change; publication uncertainty and unwinding fence the service. Scratch is
logically cleared on every returning path and unwind. The
[Bounded File Backend Log](../Development/2026-10-13%20Bounded%20File%20Backend%20Log.md)
records the separate host qualification and integration limits.

Two bounded-source clients reconstructed the complete pinned libc. Independent
verification checked 2,755 operations, while all 29 required/affected commands
and fresh TCG/KVM 96/320 MiB runs passed. All five development images and the
native source-read artifacts remain identical. Peak guarded memory was 2.57 GiB,
with no observed OOM or pressure abort. That was a host-qualified backend
checkpoint; its native guest still made only the existing three Linux returns.

The native integration under
[ADR 0087](../../docs/adr/0087-return-native-file-service-replies-to-the-linux-loader.md)
now opens libc as fd 3 and returns its first 832 bytes to the unchanged Linux
interpreter. Userspace namespace policy supplies real absence decisions for
missing candidates, including a bounded Linux trailing-slash metadata adapter.
Exact native context and delayed acknowledgements bind portable state changes
to the actual caller copyout and reply. At that checkpoint the next observed
unsupported request was `pread64(fd=3, count=840, offset=64)` after eight calls
and seven returns.

The same retained image passes TCG/KVM at 96/320 MiB with three fresh lifetimes:
normal publication, fault after stage, and unknown local result after accepted
publication. Bytes are copied exactly once where accepted; faults retire the
dependent pair, preserve an independent peer, clear private/staged state and
restore ownership. Six malformed-source cases pass. VirtualBox again refuses
`application_cpu_amd_error_pointers` before userspace and is powered off.
All 29 required/affected checks passed and the CPU workspace image is restored.
Four development images remain byte-identical; the editor image has only
individually decoded panic line-number changes and corresponding checksums.
No OOM was observed. The
[Native File Replies Log](../Development/2026-10-13%20Native%20File%20Replies%20Log.md)
records both earlier failed guest runs, source/artifact pins, emitted reviews and
qualification limits.

The positional-read integration under
[ADR 0088](../../docs/adr/0088-read-linux-files-without-advancing-descriptor-offsets.md)
now returns the 840-byte libc program-header range at offset 64. The native veil
uses the existing authenticated source/stage/reply operations, while the portable
service commits watermark 8 and preserves fd 3's sequential offset of 832.
At that checkpoint the next unsupported request was `fstat(fd=3)`, syscall 5,
after nine calls and eight Linux returns.

The same reviewed guest artifacts pass TCG/KVM at 96/320 MiB with five fresh
lifetimes, covering normal delivery plus stage-fault and unknown-local-result
cases for both sequential and positional reads. Independent host verification
checks both retained delivery journals against exact source bytes, actual caller
memory and complete saved frames. Six source-provisioning refusals pass;
VirtualBox retains its CPU capability refusal and verified poweroff. All 29
required/affected checks and final CPU workspace image restoration pass. All
five development baseline images remain byte-identical. Peak guarded memory is
2.60 GiB, with no observed OOM or pressure abort. The
[Positional File Reads Log](../Development/2026-10-13%20Positional%20File%20Reads%20Log.md)
records the source/artifact pins, emitted reviews and qualification limits.

The open-file metadata integration under
[ADR 0089](../../docs/adr/0089-publish-open-file-metadata-through-the-linux-veil.md)
now publishes the retained snapshot's 144-byte Linux stat record with result
zero and no source read. The unchanged interpreter accepts it, repeats its
positional program-header read, then reaches syscall 9, `mmap`, at sequence 11.
The observed request is `mmap(NULL, 2231696, PROT_READ,
MAP_PRIVATE | MAP_DENYWRITE, 3, 0)`. Ten Linux returns precede the unsupported
mapping request; the sequential offset remains 832 and the service watermark
reaches 10.

Identical reviewed artifacts pass TCG/KVM at 96/320 MiB across seven fresh
lifetimes. Four retained normal deliveries include the repeated read; each
nonempty publication runs its own rejection checks. Independent verification
checks every metadata byte, actual caller memory, complete frames and all fault
cleanup. Six source-provisioning refusals, all 29 required/affected checks and
CPU workspace image restoration pass. All five development baseline images
remain byte-identical. Peak guarded memory is 2.18 GiB with no observed OOM
or pressure abort. VirtualBox's CPU capability refusal and verified poweroff
remain separate. The
[Open File Metadata Log](../Development/2026-10-13%20Open%20File%20Metadata%20Log.md)
records the failed first boot, correction, exact evidence and next-slice handoff.

The next ownership prerequisite is implemented and host-qualified under
[ADR 0090](../../docs/adr/0090-retain-runtime-mapping-transactions.md): an isolated
runtime mapping transaction with retained backing, complete virtual ownership,
and explicit publication/withdrawal. Fifteen new transaction tests, the 626-test
kernel suite, strict lint and all 29 required/affected checks passed. The final
audit retains failed attempts, including a guarded host-pressure abort, and
records zero observed OOM kills. This dormant owner does not change the loader's
observed syscall boundary. The
[Runtime Mapping Transactions Log](../Development/2026-10-13%20Runtime%20Mapping%20Transactions%20Log.md)
records that work separately from the preceding guest qualification.

[ADR 0091](../../docs/adr/0091-release-allocator-borrows-before-userspace.md)
now qualifies releasing the bootstrap allocator borrow before guest entry and
ordinary reacquisition from an authenticated native service. Fresh emitted
reviews and TCG/KVM at 96/320 MiB pass all seven lifetimes: two service borrows,
one nested refusal, preserved caller state and restored resources. All thirty
required/affected regressions and six provisioning refusals pass. The final
audit preserves failed attempts and records no new observed OOM kills; the
regression build triggered soft-limit reclaim without a hard-cap event.
VirtualBox retains its separate CPU capability refusal and verified poweroff.
This qualification uses isolated code checkpoint `6e30506`; Claude's later
ADR 0094 shared-core work is outside it. The
[Allocator Service Lifetime Log](../Development/2026-10-13%20Allocator%20Service%20Lifetime%20Log.md)
records the exact evidence and unchanged first-libc-mapping boundary.

Accepted [ADR 0095](../../docs/adr/0095-prepare-and-cancel-authenticated-source-mappings.md)
qualifies authenticated source-mapping preparation and cancellation while the
Linux caller remains suspended. Native operation 25 prepares an isolated,
unpublished tree at 4 GiB: 535 populated pages within a 545-page virtual extent,
five table frames, 752 zero-padding bytes and ten absent tail pages. The normal
lifetime records one preparation and nine refusals, including the rejected
replay, with complete cancellation, restored accounting and preserved caller
state. The six file-service fault lifetimes perform no mapping work. The older
two allocator borrows and one nested denial remain separate observations.

Fresh kernel and native emitted reviews, four TCG/KVM boots at 96/320 MiB,
independent physical verification, six provisioning refusals and all thirty
required/affected regression jobs pass. The first KVM attempt's guest timeout
and the first regression attempt's outer deadline during kernel lint remain
failed evidence. VirtualBox retains its CPU capability refusal and verified
poweroff, without a loader-execution pass. The
[Live Mapping Preparation Log](../Development/2026-10-15%20Live%20Mapping%20Preparation%20Log.md)
records the exact code, artifacts, reviews, attempts and memory audit.

Accepted [ADR 0096](../../docs/adr/0096-retain-publish-and-withdraw-blocked-caller-mappings.md)
retains the actual source owner across native calls, publishes one empty caller
PDPT slot at 4 GiB, inspects through composed user-copy, then withdraws and
releases it. Ownership survives publication and uncertain outcomes; original-tree
verification and translation invalidation precede lease release and source
recovery. Operations 26–29 add thirteen calls: four successes and nine refusals.
Operation 25 retains its preparation/cancellation contract.

Fresh kernel and native emitted reviews, four TCG/KVM boots at 96/320 MiB,
independent physical verification, six provisioning refusals and all thirty
required/affected regression commands pass. The first formatting attempt remains
failed evidence. VirtualBox separately refuses `application_cpu_amd_error_pointers`
and is verified powered off; it has no loader-execution pass. The final audit
records fifteen guarded scopes and sixty attempted jobs, no overlapping guarded
job intervals and no observed OOM kill. Soft-limit reclaim occurred during
`make check`, without a hard-cap event.
The normal image is byte-identical to its prior baseline. Four development images
have explicitly reviewed source-location and Threshold hash differences; their
current hashes and coverage limits are recorded in the
[Mapping Publication Log](../Development/2026-10-15%20Mapping%20Publication%20Log.md).

Accepted [ADR 0097](../../docs/adr/0097-resume-a-caller-with-a-retained-source-mapping.md)
qualifies the exact first libc mmap return and
resumed interpreter execution with retained source and mapping ownership. Four
TCG/KVM boots at 96/320 MiB cover eight lifetimes, including an exact veil fault
after the fresh request. Independent transcript verification, 22 malformed-trace
checks, six provisioning refusals and all thirty required/affected make commands
pass. Both exact additional workspace runtime checks and the final audit pass.
The audit retains 22 guarded scopes and 75 attempted jobs, including six failed
scopes, with no overlap or observed per-scope OOM delta. Soft-limit reclaim
occurred during `make check` without a hard-cap event; the two recorded host
boots do not establish absence of intervening host OOMs.

At that historical checkpoint, the caller was suspended at sequence 12 after
eleven Linux returns. Its observed request was
`mmap(0x100028000, 0x198000, 5, 0x812, 3, 0x28000)`, requesting a
fixed private read/execute mapping over part of the retained libc range. No
policy was applied to that request. The current re-pinned length is recorded at
the top of this note. The normal and workspace-development images
remain byte-identical; three other development images have broad kernel changes.
Explicit source and exact runtime review accept bounded regression coverage,
without a metadata-only, binary-equivalence or inherited stack-bound claim.
The [Mapping Resume Log](../Development/2026-10-18%20Mapping%20Resume%20Log.md)
records current artifacts, authorship limits, retained failures, the separate
VirtualBox CPU capability refusal and two boot-local resource baselines.
Foreign syscall and ELF policy remain in userspace. Reaching the new request
proves resumption but does not prove a candidate leaf acquired Accessed.
Further mappings/protection changes, dependency loading, TLS/libc startup and the
Electron process/file/display environment remain open. The existing Linux
Antimatter build is not running inside Anomalous; no native frontend rewrite is
required.

The user named native Antimatter running and rendering **product Milestone 1**
on 2026-10-04, allowing smaller applications to validate the path. The canonical
six-app launcher and simplified bottom panel are now qualified, including 465
pointer/Appearance frames, 26 Graphics frames and a matching VirtualBox initial
frame. Editor Lab adds 95 physical-editing frames under the gate described below.
The [Keyboard Editor Workspace Log](../Development/2026-10-05%20Keyboard%20Editor%20Workspace%20Log.md)
records the current tests and memory guard results. General executable launch remains
open. Reliquary's selected
extension is `.re`; native paths must avoid fixed global length/depth ceilings
while retaining explicit resource accounting. Accepted
[ADR 0061](../../docs/adr/0061-select-re-extension-and-path-policy.md) records
that policy without claiming a serialized format or filesystem implementation.

As of 2026-10-04, Antimatter is the official editor, Ny is the official language
direction for Anomalous and HAG, and HAG is the compute runtime intended to
replace Vulkan. Accepted
[ADR 0045](../../docs/adr/0045-select-native-antimatter-ny-and-hag-direction.md)
records this destination without claiming its implementation gates are met.
Existing verified Rust remains authoritative pending qualified replacements.

The native application ABI baseline is qualified. The selected userspace ELF
loading comparison is now qualified as a bounded proof under proposed
[ADR 0051](../../docs/adr/0051-prove-userspace-elf-loading-against-native.md).
The Linux Antimatter copy on the host outside the VM remains the behavioral
reference; explicit guest Linux startup and a bounded syscall personality are
now qualified in proposed ADR 0052's fixed record-device proof. ADR 0053
implements that policy in a separate userspace veil. ADR 0054 qualifies a
shared two-caller service. ADR 0055 now implements copied native startup and
private arenas, with bounded qualification complete. ADR 0056 implements a native
RAM document service, now qualified as a bounded proof. ADR 0057 qualifies its
bounded ELF comparison. ADR 0058 adds native editor replacement while retaining
the service; bounded qualification is complete. ADR 0059 qualifies the same
bounded retained-service replacement through the userspace ELF loader against
the native baseline. ADR 0060 starts scoped persistence with a portable durable
record/recovery model. ADR 0063 now implements a separate native disk path with
qualified normal save, full pair destruction and cold QEMU recovery. Its native
failure cuts, lost committed replies, newer unsaved edits and injected loss of
device-completion observation are now qualified as a separate bounded proof.
ADR 0065 connects physical editing and direct live rendering to that durable
service in a separately owned native pair. ADR 0062 selects the larger native
filesystem architecture without implementing it. Windows/macOS remain later work.
Antimatter is largely Electron today; the qualified bounded native service proof uses a
bounded independent Rust client under
[ADR 0050](../../docs/adr/0050-prove-a-bounded-native-application-contract.md).
It establishes no full Antimatter, Electron, or Ny runtime port. Ny still needs
a broader Anomalous target/runtime, and HAG needs a separate native device
transport. A software reference path can prove rendering; useful editing later
requires open/edit/save/relaunch with exact byte verification. The
[direction log](../Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
records the source audit, dependencies, memory limits, and proposed acceptance.
Eventual comparisons cover Windows PE/COFF with a Windows personality, macOS
Mach-O with a Darwin personality, and ELF with an explicitly chosen personality.
They compare application behavior, ownership, service results, and cleanup to
the native baseline. A container is not an execution/service ABI, and the
native Ny ELF-object proof does not establish Linux compatibility. The
qualified ELF adapter uses the native personality. The new Linux entry/device
subset is separately qualified; no production foreign veil or Electron port
is implemented. The bounded Rust frontend prototype
does not constitute the complete native Antimatter editor.

Displacer is a future desktop/customization integration direction, with the
existing theme carried forward and useful changes potentially contributed
upstream. The user explicitly deferred action: no inspection or implementation
is part of this slice. The [direction log](../Development/2026-10-03%20Native%20Antimatter%20Direction%20Log.md)
retains that intent for later work.

The retained-launch dispatch checkpoint qualified all 26 regression targets
across documented phases, including two-size TCG/KVM proofs. The new internal
application CPU/copy/fault profile passed all 28 validation targets on unchanged
frozen source. Proposed ADR 0049 qualified a pinned Ny scalar object and
inspected startup shim with all 30 targets passing on 272 unchanged frozen files.
Task-bound surface/input now supplies the qualified native baseline; workspace
persistence and full Antimatter integration need their own proofs. Antimatter remains
the first intended actual application test of the ABI; the fixture is not a
general launcher.

## Interactive native editor — public scratch gate

Editor Lab now opens through F4 or the sixth Applications entry. It accepts
physical keyboard input, renders a live caret and selection, supports deletion
and one-step undo, and publishes structured rejection/status feedback. Focused
text cannot trigger desktop shortcuts. The text is explicitly public and
memory-only; Ctrl+S reports Save unavailable, and closing discards it.

[ADR 0064](../../docs/adr/0064-add-a-public-keyboard-editor-workspace.md) records
this co-located userspace gate. It reuses acknowledged row presentation and
canonical transactions, adds no kernel operation, and leaves opened-document
save ownership unchanged. The QEMU editor trace passed 95 exact frames. All 16 required checks, existing
workspace, repeated/durable and native/ELF replacement regressions passed, and
VirtualBox matched the initial frame. Qualification and zero-OOM results are
recorded in the
[Keyboard Editor Workspace Log](../Development/2026-10-05%20Keyboard%20Editor%20Workspace%20Log.md).
The emitted user-stack bound is 61,240/65,536 bytes, so later work must preserve
that budget or explicitly requalify a changed profile.

A separately owned native interactive editor now connects physical typing and
complete live rendering to the durable document service, as described below.
The actual Antimatter Electron/frontend/runtime port remains open. A public
scratch built-in is not product Milestone 1 completion.

## Interactive native durable editor — bounded reference pair

[ADR 0065](../../docs/adr/0065-prove-an-interactive-native-durable-editor.md)
adds an isolated native editor/service pair. The editor owns keyboard input and
presentation; only the service owns the provisioned storage grant. Exact pending
save bytes remain retained while newer edits continue. A late reply leaves newer
text dirty, and service retirement leaves an unknown outcome visible through
presentation that no longer depends on the service.

The physical proof passed eight QEMU boots at 96 and 320 MiB, checking 62 complete
frames and 48 stable captures. It verifies delayed acknowledgements, a second
save, explicit discard of uncertain edits, and cold recovery without supplying
the guest its expected answer. The host independently checks both committed disk
checkpoints and extent guards. All 16 required repository checks passed. Exact
emitted review bounds the client stack at 4,928/8,192 bytes and the kernel
transition plus nested idle IRQ at 9,662/16,384 bytes. The
[Interactive Durable Editor Log](../Development/2026-10-05%20Interactive%20Durable%20Editor%20Log.md)
records artifacts, validation, resource measurements and explicit limits.

This remains a public, bounded Rust reference client with a 31-byte ASCII line,
not the Antimatter Electron frontend or a general filesystem. Desktop F4 Editor
Lab remains memory-only. ADR 0066 now implements the portable workspace window,
input and semantic-observation bridge described below. Native workspace
execution admission and cross-task integration come before the same live
contract through the userspace ELF loader. Neither copied desktop views nor an
ELF container may become a second save owner or bypass document disclosure policy.

## Portable editor workspace bridge — host foundation

The default-off `native_editor_bridge` feature connects a separate session owner
to a Native Editor window under
[ADR 0066](../../docs/adr/0066-model-an-owner-bound-editor-workspace-bridge.md).
For this window, the copyable desktop holds geometry and surface metadata. A
separate sidecar retains the authorized ANUI observation and immutable pixels,
while the application owner
alone retains save state. Combined publication tickets preserve exact pixels
and semantics across rejection/cancellation. One-shot input permits reject stale
focus, grants, window incarnations and source observations. Generic window close
cannot bypass editor confirmation; explicit terminal withdrawal is separate.

Focused host tests and warnings-denied Clippy passed, including real durable
save/recovery and complete-frame composition. All 16 required checks and both
workspace QEMU regressions passed. An isolated VirtualBox initial frame matched
all 786,432 QEMU pixels. The 23 listed default artifacts preserve exact runtime
images, payloads and ELF loaded content; two whole ELFs differ only in nonloaded
debug sections. A hosted preview renders a granted edit and real dirty state.
Guarded runs peaked at 2.074 GiB with no OOM events. The new feature is not enabled
in native images, and the launcher still contains the six established built-ins.
The actual Electron frontend remains unported. The
[Editor Workspace Bridge Log](../Development/2026-10-05%20Editor%20Workspace%20Bridge%20Log.md)
records the gate's evidence and limits.

## Retained workspace — native admission gate

[ADR 0067](../../docs/adr/0067-qualify-retained-native-workspace-admission.md)
implements a separate retained launch for the real workspace: 128 KiB RX,
32 KiB RW/NX data, a 64 KiB user stack and a private 16 KiB transition stack.
The opt-in profile preserves the GPR/CR0.TS mailbox contract and existing
desktop behavior. Guard-write and NX-execution probes retire before desktop
entry; normal exits and contained failures restore the exact bootstrap state
and owned resources. Uncertain kernel invariants retain ownership and halt.

The final emitted user bound remains 61,240/65,536 bytes. Bootstrap with IRQ
uses at most 28,191/65,536 bytes, and the gate with nested idle IRQ uses
5,310/16,384 bytes. The first guest exposed an identity-alias assumption in
the old header reader; a narrow owned-header copy fixes it without widening
legacy transport bounds. The [Owned Workspace Admission Log](../Development/2026-10-05%20Owned%20Workspace%20Admission%20Log.md)
records the failed attempt, exact corrected artifacts and qualification.

All four 96/320 MiB TCG/KVM configurations passed, totaling 32 VM boots,
100 admission/release cycles and 2,428 checked interactive frames. VirtualBox
passed guard/NX retirement and matching initial rendering. All sixteen required
checks and affected workspace/ownership regressions passed. Normal/workspace
development images remain unchanged; six older ABI kernel image deltas have
separate review. Guarded work recorded no OOM events, with a 4.001 GiB maximum
peak and 14.771 GiB minimum available host memory across all attempts.

Two editor content buffers need storage outside the nearly full workspace
stack. ADR 0070 below implements a separate nine-page arena and three retained
task owners, qualified by bounded TCG/KVM probes. Executing the real coordinator
still needs focus/input, observation and document routing without competing
framebuffer owners.
The portable bridge remains disabled in native images, so workspace admission
does not yet put the durable native reference editor inside a desktop window.

## Retained workspace — eager CPU ownership gate

[ADR 0068](../../docs/adr/0068-qualify-eager-workspace-cpu-ownership.md) qualifies
an opt-in eager x87/MMX/SSE2 owner for the real retained workspace. Every admitted
user transition captures into private resident storage, ordinary returns restore
the task image, and terminal withdrawal restores and verifies the original
bootstrap CPU state before storage/resource release. Kernel-origin idle IRQs
leave the user owner alone. The GPR-only profile remains unchanged.

Four 96/320 MiB TCG/KVM configurations passed: 32 boots, 100 complete owner
lifetimes and 2,428 checked interactive frames. All sixteen required targets
and four affected regressions passed; all six captured baseline artifacts are
byte-identical. The emitted user stack bound is 61,248/65,536 bytes, bootstrap
plus IRQ 28,247/65,536, and gate plus nested idle IRQ 5,294/16,384.

VirtualBox 7.2.6 reached the kernel but its exposed AMD CPU omits XSaveErPtr, so
the existing CPU admission rule refused the new profile before userspace entry.
This is not a first-frame qualification; normal/legacy workspace images are
unchanged. One initial build scope was killed by systemd-oomd for shared-session
pressure despite zero kernel OOM counters. Recovery added system/ancestor PSI
monitoring and one-job builds; the final scopes peaked at 1.687 GiB without
further pressure or OOM incidents. The [Eager Workspace CPU Log](../Development/2026-10-05%20Eager%20Workspace%20CPU%20Log.md)
retains exact successful and failed evidence, journal limits and source bindings.

The split coordinator below implements the portable role routing, borrowed
frame ownership and focus/publication barrier. Native mixed-owner admission is
qualified separately under ADR 0070 using bounded TCG/KVM probes. The
native bridge remains disabled; Electron Antimatter and product Milestone 1 are
still incomplete.

## Split editor coordinator — portable integration

[ADR 0069](../../docs/adr/0069-compose-a-split-editor-workspace-coordinator.md)
adds workspace/editor/document role capabilities and four directed routes to
the opt-in bridge. Submission fences desktop publication and grant changes until
the exact executed or rejected outcome returns. Workspace reply processing
remains possible; a stale action after a service acknowledgement has no effect.
Retirement closes only incident routes, preserving editor/workspace progress
after storage-service loss and accepted-save progress after workspace loss.

The same bridge can now retain two caller-supplied immutable frame buffers and
return old storage after complete publication. Its compact action/update owners
carry references and metadata, while the composed host tests use two heap-backed
18,432-byte slots. This removes pixel arrays from those value returns. ADR 0070
below qualifies their guest arena geometry and retention using small probes; the expanded real
application call stack remains to be qualified.

The new composed tests exercise the real durable store and complete compositor,
late saved-B acknowledgement while C remains dirty, immutable pending frames,
stale-action rejection, role retirement and unknown-save rendering. Focused
tests, freestanding checks and warnings-denied linting passed on `cb6eebd`.
All sixteen required targets and the 512-frame existing desktop regression
passed; four rebuilt boot images remain byte-identical. Seven guarded scopes
peaked at 2.155 GiB with no OOM kill or aborted job. A system pressure burst
delayed admission between jobs until the quiet interval recovered.
The [Split Editor Coordinator Log](../Development/2026-10-05%20Split%20Editor%20Coordinator%20Log.md)
records validation and the memory-pressure guard. ADR 0070 now implements the
mixed native admission prerequisite using small independent probes. The real
workspace/editor/service payloads and visible editor window remain the next
native integration; ADR 0071 below adds the copied host adapter; this portable gate enables no new guest
launcher entry or Electron frontend.

## Mixed native workspace admission — bounded native proof

[ADR 0070](../../docs/adr/0070-qualify-mixed-native-workspace-admission.md)
implements three isolated CPL3 probes with workspace/editor/service roles, one
retained scheduler/root/TSS/CPU owner and four task-bound directed routes. The
workspace has 32 RX, eight RW, nine arena and sixteen stack pages; the two
application roles retain their eight RX, two RW, eight arena and two stack pages.
Together they own 144 frames in eighteen leases. The arena can hold two future
18,432-byte frames, but the current probes only write and check patterns.

Each role proves distinct x87/XMM state across timer interrupts and dispatch.
The probes check denied role calls and guard-crossing copies without consuming
queued data. An exact service fault closes its empty incident routes, then the
workspace and editor exchange two more packets before complete cleanup. No
physical input, framebuffer or disk grants are installed, and no real editor,
workspace renderer or durable service payload runs in this profile.

The final candidate `a425ebd` passed four TCG/KVM boots at 96 and 320 MiB,
including full workspace arena retention after service retirement. The final
comparison preserves all original pattern bytes except the two exact
workspace/editor packet words; no refill can hide corruption. All sixteen
required checks and affected application, interactive-save/recovery and workspace
regressions passed. The gate stack bound is 5,927/16,384 bytes and bootstrap plus
one conservative IRQ is 31,759/65,536. VirtualBox 7.2.6 reached the kernel but
refused its AMD CPU profile before userspace entry because XSaveErPtr is absent;
the isolated VM powered off. The normal image is unchanged; workspace images
differ only in diagnostic line metadata and corresponding manifest hashes. No
OOM kill or guard abort was observed; the earlier full check recorded soft-limit
reclaim events. Exact comparisons and memory/journal evidence are recorded in the
[Mixed Native Workspace Log](../Development/2026-10-05%20Mixed%20Native%20Workspace%20Log.md).

## Copied remote editor adapter — host integration

[ADR 0071](../../docs/adr/0071-compose-a-copied-remote-editor-adapter.md) adds an
opt-in copied request/result protocol and remote workspace bridge. The editor
owns the sole mutable session and save snapshot. The workspace retains only
checked observations, action correlation and two borrowed frame slots. Exact
completion releases the desktop publication fence; newer observations can drain
while fenced, older completions cannot roll them back, and input waits until the
latest accepted view is displayed. Save reservation remains distinct from durable
acknowledgement. Service loss preserves unknown state and live editor/workspace
communication.

The codec and adapter passed host integration with the real editor, durable
store and complete compositor on `82ff1d9`. All sixteen required checks and the
512-frame existing desktop regression passed. Four rebuilt baseline boot images
are byte-identical; all 436 frozen source/configuration files stayed unchanged.
The three guarded scopes peaked at 3.207 GiB with no observed OOM, pressure abort
or failed job. The
[Remote Editor Adapter Log](../Development/2026-10-07%20Remote%20Editor%20Adapter%20Log.md)
records that checkpoint's evidence and memory limits. It selected no real guest
payloads. ADR 0072 below adds checked native receive bindings, workspace-only
input/display, service-specific route adaptation, emitted stack/capacity review
and a visible three-task editor window. Full Antimatter/Electron execution
remains open.

## Native editor workspace — three-owner integration

[ADR 0072](../../docs/adr/0072-integrate-a-native-editor-workspace.md) selects
real workspace, editor and document-service payloads under one native coordinator.
The workspace composes the desktop and owns physical keyboard/mouse delivery;
the editor alone owns the editing session; the service alone owns disk access.
Four copied routes carry actions, exact completions, observations and document
requests. The editor opens in a desktop window using two borrowed arena frames.

The opt-in `native-editor-workspace-build` and `native-editor-workspace-smoke`
commands inspect the three private task layouts and exercise the composed window,
delayed replies, service loss and cold recovery. The final native matrix passed
16 initial/cold boots across TCG/KVM at 96 and 320 MiB, verifying 140 complete
desktop frames and 96 stable captures. A late acknowledgement for B leaves newer
C dirty; the next save becomes clean. A service cut after committing B leaves
the save unknown while editing C continues; a fresh boot recovers B from disk.
All three owners exit and all 720 owned frames are restored. All sixteen required
checks and affected regressions passed; the four baseline images are unchanged.
No OOM kill or pressure abort was observed. An earlier check reached the 4 GiB
soft limit and reclaimed memory; the successful required run peaked at 2.94 GiB.
VirtualBox still stops at this profile's QEMU-specific fixture admission. The
[Native Editor Workspace Log](../Development/2026-10-07%20Native%20Editor%20Workspace%20Log.md)
records qualification, emitted bounds and remaining platform limits. This
bounded Rust reference is another ABI integration gate. Full Electron
Antimatter, a general executable launcher and product Milestone 1 remain open.

## Native filesystem — architecture selected, implementation pending

There is no general filesystem or namespace resolver. Threshold's boot reads
do not provide runtime writes. ADR 0063 now adds an opt-in native editor/service
fixture using a dedicated disk; earlier document profiles remain RAM-only.
ADR 0060's alternating checkpoints model interrupted-update recovery on one
device, not independent redundancy.

Accepted [ADR 0062](../../docs/adr/0062-design-native-object-filesystem.md) and
the [filesystem architecture](../../docs/architecture/filesystem.md) select
stable objects, immutable versions, explicit views and live grants; streamed
names/traversal without fixed component/path/depth ceilings; copy-on-write
project transactions; snapshots; independently restorable backups; and later
verified mirror repair. These are requirements, not implemented capabilities
or a stabilized disk format. Fresh authority after backup rollback must come
from a qualified source outside that backup.

Proposed [ADR 0063](../../docs/adr/0063-prove-native-durable-document-storage.md)
now qualifies normal native save of a run-specific challenge to a dedicated
data disk, destruction of both editor and service, remount through fresh owners,
and cold recovery in a new QEMU process. The recovery VM receives no expected
text. Both 96 MiB and 320 MiB runs pass independent media/transcript checks;
a capture verifies 9,216 public text pixels. Blank/corrupt recovery and nonblank
formatting fail closed without writes. The boot disk and outside-extent bytes
remain unchanged. The temporary feature-private ATA PIO mechanism handles only
opaque sectors; document policy remains in userspace. The separate native
failure proof now qualifies service cuts before/after save operations, lost
committed replies, newer edits that remain dirty, and injected loss of device
completion observation with retained quarantine. It passed 28 debug-exit boots
and fourteen public-frame captures, with independent cold recovery and no
expected text supplied to recovery. This is not abrupt VM/host power-loss or
physical hardware qualification. See the
[Native Durable Failure Log](../Development/2026-10-04%20Native%20Durable%20Failure%20Log.md)
and the
[Native Durable Document Log](../Development/2026-10-04%20Native%20Durable%20Document%20Log.md)
for exact artifacts, checks and limitations.
The failure-proof checkpoint passed all 16 required checks, native/ELF replacement
and workspace regressions. Its final artifact/stack reviews were bound to the
tested binaries; an isolated VirtualBox workspace matched all 786,432 QEMU
pixels. Ordinary boot/workspace images matched the preceding baseline, and that
checkpoint's replacement image differences were confined to diagnostic lines
and checksums. No OOM or hard-limit events occurred; the 4 GiB soft guard
reclaimed memory during its large matrix.

Repeated local editing is now qualified through the opt-in
`repeated_opened_editor_state`: each successful edit retains its immediate
predecessor and reports any discarded older undo entry. Rejected edits leave
history, saved state and pending saves intact. The established editor types keep
their original rejecting policy and ABI behavior. The new native proof builds a
challenge through three consecutive edits, makes a newer edit while a save is
pending, preserves dirty state after that late acknowledgement, then saves,
undoes, edits and saves again. Content version advances from one to four.
Same-VM remount and cold recovery at 96/320 MiB recover the final bytes without a
recovery answer in startup data. Four debug-exit boots and two independent public
frame captures pass. This reuses the existing durable kernel runtime; there is
no new privileged operation, stable wire format, filesystem or global path limit.
See the [native editor contract](../../docs/architecture/native-editor.md) and
[Repeated Native Editing Log](../Development/2026-10-04%20Repeated%20Native%20Editing%20Log.md)
for retention policy, exact artifacts and qualification. All 16 required checks
and the five additional durable/native/ELF/workspace regressions passed on the
final source. Exact native rebuilds match the tested binaries; a private
VirtualBox workspace matched all 786,432 QEMU pixels. Normal/workspace and old
durable/failure images remain unchanged. Replacement clients received a fresh
stack/control review for a helper-function reorder; privileged executable bytes
are unchanged. No OOM or hard-limit events occurred across nine guarded scopes.
Implementation checkpoint: `9d03b46`.

Physical keyboard focus and a live editor window remain the next integration
gate. This is a scripted Rust client with one printable ASCII line and one-step
undo, not the full Electron/React/CodeMirror Antimatter frontend. The Linux
Antimatter checkout remains the behavioral reference and is left untouched.
The existing workspace already receives physical input, but an editor must
first define focus/shortcut precedence, report rejected edits semantically and
connect to real document service state before offering Save. The
[Filesystem Architecture Log](../Development/2026-10-04%20Filesystem%20Architecture%20Log.md)
records the broader filesystem requirements and remaining implementation gates.

## Durable document foundation and VirtualBox — portable qualification complete

Proposed [ADR 0060](../../docs/adr/0060-model-bounded-durable-document-recovery.md)
adds a separate safe `no_std` five-sector document store. It commits a fresh
mount epoch before granting handles and acknowledges a save only after body and
commit flushes. Uncertain I/O quarantines the store. Independent crash tests
cover operation failures, torn writes, partial flushes and stale identities.
All 16 required repository checks passed, along with native/ELF replacement and
workspace regressions. The final document suite passed 28 unit and ten crash
tests. The isolated VirtualBox runner passed 13 host tests and four live probes
while the ordinary background service remained active. No OOM, soft-limit or
hard-limit events occurred; peak scope memory was 2.85 GiB. Normal, workspace
and both replacement images remain byte-for-byte unchanged.
This checkpoint qualified portable policy over a sector adapter. The original
guest clients remain RAM-only; ADR 0063's separate native disk profile now
qualifies normal save and full restart recovery as described above.

Actual VirtualBox 7.2.6 probes booted and rendered the normal shell and workspace
desktop. The earlier direct `/dev/vboxdrv` permission check was insufficient:
the installed launcher starts correctly without host permission changes.
Both replacement proofs safely reject VirtualBox's masked AMD
`XSaveErPtr` capability. The kernel guard is preserved; newer native/ELF
application tests continue through QEMU pending separate VirtualBox CPU-state
qualification. `make workspace-virtualbox` remains the interactive desktop
entry point. The isolated proof runner retains captures and diagnostics without
adopting the development VM. The existing **Anomalous Dev [Threshold]** VM is
refreshed with the workspace disk and ready for interactive testing. See the
[Persistence Foundation and VirtualBox Log](../Development/2026-10-04%20Persistence%20Foundation%20and%20VirtualBox%20Log.md)
for exact scope, evidence, host limits and the CPU-mask investigation.

## ELF editor replacement comparison — bounded qualification complete

Proposed [ADR 0059](../../docs/adr/0059-prove-elf-editor-replacement-against-the-native-baseline.md)
adds `elf_relaunch_test` around the qualified native replacement client and one
shared retained runtime. Two loader tasks offer one inspected inert plan and
retire completely before adjacent native/ELF runs of all three replacement
scenarios. Each initial and replacement construction uses its selected image.
Verified zero-tail compaction preserves the native virtual RW bytes and existing
loader bounds; the native `int 0xf4` personality remains unchanged.

The independent oracle requires 222 ordered records, including 48 surfaces and
138 document packet hashes, plus exact maintenance continuity, resource counts
and fresh authority. Both routes must match known native checkpoints; agreement
between two wrong routes is insufficient. All 52 qualification checks and two
development-image restorations passed with 369 frozen source files unchanged.
TCG/Intel and KVM/AMD passed at 96 MiB and 320 MiB with matching captures. Direct
comparison also matched all 99 lifecycle, surface and packet records per route
to the qualified native baseline in all six backend/size runs, excluding only
the event prefix, route label and individually bounded timer counts.

Each complete proof executed eighteen document task lifetimes across six
retained services and six replacements, with six additional unstarted candidates
cancelled. Four editor faults were contained and 589,824 arena bytes were
freshly zeroed. Host checks passed 491 kernel tests for each replacement profile,
531 all-features kernel tests and 221 xtask tests. Exact emitted reviews, prior
baseline preservation and full qualification receipts are retained in the
[ELF Relaunch Log](../Development/2026-10-04%20ELF%20Relaunch%20Log.md).
Commands are `make elf-relaunch-inspect`, `make elf-relaunch-image` and
`make elf-relaunch-smoke`. This introduces no foreign personality, arbitrary ELF
admission or persistence beyond the retained RAM service. Scoped filesystem
durability follows the comparison.

## Native editor replacement — bounded qualification complete

Proposed [ADR 0058](../../docs/adr/0058-prove-native-editor-replacement-with-a-retained-document-service.md)
adds the `native_relaunch_test` proof and a separate native Rust client. One RAM
document service retains its task, circle, mappings, saved frame, private memory
and CPU image while the controller destructively retires and replaces an editor
on the original kernel stack. The service applies the new document-authority
binding in userspace after receiving checked successor metadata.

The three cases replace the editor after an acknowledged save followed by
unsaved edits, a fault before a second queued save executes, and a fault after
that save commits but before its reply is consumed. Fresh opens must recover
the actual committed bytes/version. Old routes and handles are revoked; the new
editor starts with zeroed private storage and performs another save/read/reopen
sequence. The proof includes owner-specific cleanup and construction retries
while the same service remains retained.

The independent oracle requires 122 ordered records, including 24 surface
checkpoints and 69 document packet hashes across both editor incarnations.
All 50 qualification checks and two development-image restorations passed.
TCG/Intel and KVM/AMD passed at 96 MiB and 320 MiB with matching captures. Within
the new profile, nine executed task lifetimes cover three retained services and
three editor replacements; three committed but unstarted candidates are cancelled
separately. Two editor faults are contained, and 294,912 arena bytes are verified
freshly zeroed.

Host tests passed: 491 for the relaunch kernel, 531 with all kernel features, 207
for tooling, and 18 each for the editor and document protocol. All 366 frozen
source files remained unchanged during qualification. Fourteen earlier payload
blobs, eight earlier capture pairs and the normal/workspace images are unchanged.
There were no OOM or hard-limit events; 4,398 recorded 4 GiB soft-limit events
represent memory throttling. The
[Native Relaunch Log](../Development/2026-10-04%20Native%20Relaunch%20Log.md)
records final receipts and emitted-code reviews. Commands are `make native-relaunch-inspect`,
`make native-relaunch-image` and `make native-relaunch-smoke`.

This retains the native `int 0xf4` personality and fixed mapping bounds. It adds
no filesystem, durability across service/machine restart, general process
manager or full Antimatter/Electron port. ADR 0059 qualifies the matching bounded
userspace ELF replacement comparison above. Scoped filesystem persistence with
explicit durability semantics is the next separate gate.

## ELF document comparison — bounded qualification complete

Proposed [ADR 0057](../../docs/adr/0057-prove-elf-loading-of-the-native-document-contract.md)
compares the qualified document client through native flat spans and guest-parsed
ELF spans, retaining the native `int 0xf4` personality. The full native client
and RX bytes remain unchanged. A profile-specific compactor converts a verified
zero RW file tail to zero-filled memory and checks the complete virtual image,
keeping the existing parser, inert plan and 32 KiB loader bound.

Both routes independently passed the existing document wire oracle,
uncertain-save fault cases, cleanup and final-pixel checks. All 48 qualification
checks and two development-image restorations passed, including TCG/Intel and
KVM/AMD at 96 MiB and 320 MiB with captures. The
[ELF Document Log](../Development/2026-10-04%20ELF%20Document%20Log.md)
records the final receipts. Use `make elf-document-inspect`,
`make elf-document-image` and `make elf-document-smoke`. This introduces no
Linux personality, arbitrary ELF admission or persistence beyond a live RAM
service. ADR 0058 adds native editor replacement with the service retained,
with bounded qualification complete. ADR 0059 now qualifies retained-service
replacement through the userspace ELF loader against the native baseline. Scoped
filesystem persistence with explicit durability semantics follows.

## Native document open/save — bounded qualification complete

Proposed [ADR 0056](../../docs/adr/0056-prove-task-bound-native-document-open-and-save.md) adds an independent
RAM document/render service. The editor receives document bytes through open,
submits an immutable save snapshot, makes newer edits before the reply, and
closes/reopens the committed bytes under a fresh handle. Four task-bound copied
routes separate surface, input, document requests and replies. Kernel code
continues to own copying, identity, scheduling and fault containment; document
records and save policy remain in userspace.

A lost reply leaves the save unknown and the view unsynchronized/dirty, including
when undo returned to the last acknowledged bytes. Negative retry replies do
not prove that an earlier save failed to commit. Fault cases stop the service
before commit and after commit but before reply; the surviving editor keeps
its pending snapshot. A fresh pair starts a new RAM store and seed.

Bounded native qualification passed, including the actual host Antimatter
save-store reference and native execution checks. The
[Native Document Log](../Development/2026-10-04%20Native%20Document%20Log.md)
records the qualification and memory/artifact receipts. Commands are
`make native-document-inspect`, `make native-document-image` and
`make native-document-smoke`. Bytes survive close/reopen only inside the same
live service. This does not prove saved data across editor/service destruction,
a filesystem, durable storage or full Antimatter/Electron. ADR 0057 qualifies
the contract's bounded ELF comparison. ADR 0058 adds native editor replacement
with the service retained, with bounded qualification complete. ADR 0059 qualifies
that replacement through the userspace ELF loader against the native baseline.
Scoped filesystem persistence with explicit durability semantics follows.

## Native startup and private arenas — bounded qualification complete

Proposed [ADR 0055](../../docs/adr/0055-prove-native-startup-and-private-userspace-arenas.md) implements a copied,
versioned 128-byte startup descriptor and a guarded 32 KiB private arena per
task. Supplied document bytes establish the editor's clean baseline; userspace
allocates editor state and render/transport buffers from that arena. Opaque
handles are checked against live task/channel bindings, and the descriptor
cannot create authority. An explicit four-region launch/circle policy preserves
the qualified three-region profiles.

The new proof includes two documents and arena layouts, guard-write and NX
fault containment, complete ownership return and fresh zeroed arenas.
Its execution order places the second normal document after the fault cases.
**All 43 qualification checks and two image restorations passed**; measured results and failed attempts are retained
in the [Native Runtime Log](../Development/2026-10-04%20Native%20Runtime%20Log.md). The commands are
`make native-runtime-inspect`, `make native-runtime-image` and
`make native-runtime-smoke`.

ADR 0056 now implements the bounded native RAM document follow-up described
above, now qualified. ADR 0057 also qualifies the bounded contract through ELF.
ADR 0058 qualifies native editor replacement with the service retained. The
same retained-service replacement through the userspace ELF
loader is qualified by ADR 0059; scoped filesystem persistence with explicit
durability semantics follows. A general runtime and
full Antimatter/Electron remain open; Ny/HAG retain
their separate gates.

## Shared two-caller userspace veil — completed checkpoint

The preceding proposed [ADR 0054](../../docs/adr/0054-prove-a-shared-multi-caller-userspace-veil.md) qualifies two independent Linux editor
producers and two native services sharing one supervised veil. Exact sealed
membership, per-caller routes and copied-memory isolation preserve the native
baseline. A caller fault leaves its peer progressing; a veil fault revokes both
dependents and discards its staged effect while both native services survive.
All 41 qualification checks and two image restorations passed, including TCG/Intel and
KVM/AMD at both guest sizes with capture comparison. All twelve preceding
embedded artifact blobs remain unchanged and no OOM occurred. The
[Shared Veil Log](../Development/2026-10-04%20Shared%20Veil%20Log.md) records the frozen-source and compiled-code evidence.
This is a fixed two-member serial service, not full Antimatter/Electron or a
general process/runtime service.

## Supervised userspace syscall veil — preceding increment

Proposed [ADR 0053](../../docs/adr/0053-prove-a-supervised-userspace-syscall-veil.md) adds an independent
native-entry veil beside the unchanged Linux producer and native service.
Syscall-number normalization, FD routing, errno and poll policy move out of the
kernel adapter. The kernel forwards raw arguments in a copied 80-byte request,
blocks the exact caller and selects its sealed veil while keeping the service
queued. Three independent roots, transition stacks and eager CPU images remain
owned through terminal cleanup.

The veil can request bounded checked reads from its suspended caller and one
inert submit, consume or copyout stage. Completion checks the request, stage,
return frame and prepared scheduler reply before any effect. Rejected replies
leave the original stage usable. A veil fault discards the stage, revokes the
call authority and contains its dependent caller while the native service
continues. The three qualified scenarios preserve the seven-state native editor
comparison and cover queued producer fault and unpublished staged-veil fault.

TCG and KVM runs at 96/320 MiB and separate captures passed the exact 26-record
oracle, including restoration of all 75 frames and unpublished-stage abort.
All 39 matrix jobs and two restorations passed on 325 unchanged frozen source files, with zero OOM events. The
[Userspace Veil Log](../Development/2026-10-04%20Userspace%20Veil%20Log.md)
records the final regression matrix, TCG/KVM comparison, emitted stack
review, artifact identity and restoration evidence. ADR 0053 remains Proposed.
This is one fixed caller/veil relation; initial Linux stack construction remains
in the proof controller, and this profile does not rerun the separately
qualified ELF loader. ADR 0054 subsequently qualified multi-caller supervision;
ADR 0055 implements bounded native startup and arenas with bounded qualification complete. Full Electron, libc, signals,
TLS and general process admission remain open.

## Linux startup and record devices — completed checkpoint

Proposed [ADR 0052](../../docs/adr/0052-prove-linux-startup-and-record-device-syscalls.md)
adds a real Linux x86-64 startup/SYSCALL producer paired with the existing native
INT f4 service. Its minimal owned stack carries two arguments, empty envp and
only AT_PAGESZ/AT_ENTRY auxiliary metadata. An owned entry session captures and
restores EFER/STAR/LSTAR/FMASK, selects the trusted supervisor stack before any
push and enables SCE only for the Linux producer.

The fixed read/write/poll/yield/exit subset exposes FD3 surface output and FD4
input as nonblocking Anomalous record devices, explicitly not Linux pipes.
Generic atomic channel consumption avoids a fallible acknowledgement after a
visible read; complete copy preflight preserves records and poll-array outputs
on failure. Editor semantics remain in userspace. The normal and queued-UD2
routes reproduce the same seven states, six commands, pixels and resource cleanup
as the qualified native baseline.

Qualification completed on 2026-10-04: all 37 matrix jobs and two restoration
jobs passed on 313 unchanged frozen source files. The full check passed 469
Linux-profile kernel tests, 503 all-feature kernel tests, 149 xtask tests and
nine shared ELF parser tests. TCG and actual AMD-host KVM passed at 96/320 MiB,
with identical kernel/image bytes and exact whole-frame captures. Each boot
verified 25 records, normal/fault call counts of 132/12, and restoration of
24,113/81,457 free frames. Normal and workspace images were restored unchanged.
The final matrix peaked at 2.310 GiB; available host memory stayed above
20.222 GiB, all scope events remained zero and there were no OOM kills.
The [Linux Application Log](../Development/2026-10-04%20Linux%20Application%20Log.md)
records the exact subset, artifacts, emitted-code review and final evidence. This
feature-private proof-controller bridge is not a production userspace veil;
its next policy-placement gate is the ADR 0053 follow-up above. It
establishes no stock libc, signals, TLS, arbitrary Linux execution or Electron compatibility.
The preceding ELF qualification below remains completed historical evidence.

## Userspace ELF loading — completed checkpoint

Proposed [ADR 0051](../../docs/adr/0051-prove-userspace-elf-loading-against-native.md)
adds an independent ring-3 loader for a canonical sectionless ELF64 image. The
allocation-free, unsafe-free parser emits a 96-byte inert plan for fixed RX/RW
extents. The loader submits that plan through the checked-copy boundary; the
kernel validates its geometry, exact trusted-manifest equality and current
loader role. ELF headers are parsed only in userspace. Wrong roles, bad copies,
malformed or unauthorized plans and duplicate offers must leave admission
unchanged. Loader tasks and all resources retire before application allocation.

The same inspected editor/service client then runs through native flat spans
and the ELF-derived spans. Both routes use native `int 0xf4` startup/services,
the retained CPU/copy/ownership profile, and the normal and queued-packet fault
scenarios. Initialized writable bytes are copied and read back, a nonzero canary
is read by each client, and BSS, page slack and stacks are checked zero. The
comparison covers service counters, seven surface hashes, final framebuffer
bytes, fault-preserved presentation and complete cleanup. Timer counts are not
required to match. Two loader tasks and eight application tasks run in
successive pairs, with one final-frame presentation per application route.

Qualification completed on 2026-10-04. All 35 matrix jobs and both image
restoration jobs passed with 302 frozen source files unchanged. This includes
`make check` and the ELF/native proofs at 96 MiB and 320 MiB under TCG and
actual-host KVM, plus separate captures. Proof kernel/image bytes are identical
across backends, and the editor pixel bytes match exactly. Normal and workspace
images were restored unchanged from the preceding native checkpoint.

Final-matrix peak scope memory was 2,045,624,320 bytes (1.905 GiB); the host
retained at least 21,207,310,336 bytes (19.751 GiB) available. All monitored
scope event counters remained zero, and the host OOM counter stayed at zero.
ADR 0051 remains Proposed. The [ELF Application Log](../Development/2026-10-03%20ELF%20Application%20Log.md)
records the implementation, initial failures, memory limits and final evidence.
Use `make elf-application-inspect`,
`make elf-application-image` and `make elf-application-smoke` for this separate
profile. It does not admit arbitrary files or implement Linux/Electron.
The separate Linux startup/syscall proof under ADR 0052 is now qualified
above; Ny, HAG and full Antimatter integration retain their separate gates.

## Native application contract — preceding increment

Proposed [ADR 0050](../../docs/adr/0050-prove-a-bounded-native-application-contract.md)
implements a bounded Rust editor client and independent surface/input service.
The allocation-free model ports Antimatter's actual selection replacement,
theme preservation and undo behavior, with 31 printable ASCII bytes and a
128×8 RGB8 renderer. Six scripted commands produce seven copied surfaces.
Channels bind exact task generations, preserve failed-copy retries and revoke
before terminal transitions. The service retains its final copy after producer
exit; both tasks retire before one verified bootstrap framebuffer commit.
A separate producer-fault scenario discards its queued surface and preserves
the earlier frame. That checkpoint did not parse ELF at runtime and does not
provide physical input, an interactive compositor or a full Electron/Ny frontend port.

All 33 matrix jobs and both restoration jobs passed on 291 unchanged frozen
files. Native TCG/KVM passed both memory sizes and captures with identical
images and editor pixels. Peak scope memory was 4.002 GiB under the 6 GiB cap,
with no OOM event. Normal boot is byte-identical; the shared presenter changed
the workspace image, whose TCG/KVM smoke tests passed. The
[Native Application Log](../Development/2026-10-03%20Native%20Application%20Log.md)
records final evidence, scope and remaining gates. The host command
`node scripts/native-application-reference.cjs /path/to/exec-antimatter`
compares the checked trace with that checkout's actual CodeMirror state and
production editor bridge; its result does not imply guest Linux compatibility.

## Ny object — preceding increment

Proposed
[ADR 0049](../../docs/adr/0049-qualify-a-bounded-freestanding-ny-object.md)
uses the immutable pinned Ny SDK to compile one scalar fixture, preserving the
generated object and inspecting its instructions, imports, relocations and linked
extents. A narrow assembly shim calls the generated C entry and supplies its
four observed runtime hooks. Unused string-helper imports receive fatal trap
stubs; the hosted runtime is not linked.

The fixture completed two pairs of private owners. One first task takes Ny's
generated ABI-mismatch branch; another completes its computation before an
injected context-end failure. Each surviving peer computes successfully. The
source checks initialized integer/f64 globals and 50,000,000 exact iterations;
startup checks copied nonzero header data, BSS zero-fill and stack alignment.
Actual timer observations fell inside generated code. Messages/results
cross current-task checked copies, and owners remain live through terminal
cleanup and kernel CPU restoration.

All 30 final targets passed in 1,293.38 seconds of summed target time, with all
272 frozen non-documentation files unchanged through image restoration.
`make check` took 270.07 seconds and passed 440 default, 449 Ny-feature and 483
all-feature kernel tests, plus 119 xtask tests. TCG Intel-baseline and actual
AMD-host KVM runs at 96 MiB and 320 MiB each accepted 13 records, with four tasks,
four dispatches, two faults, two normal exits and no scheduler preemptions.
Every run restored exactly 24,149/81,493 free frames. Timer observations vary
by backend and are not benchmarks.

Peak scope memory was 4,297,084,928 bytes (about 4.002 GiB) under the 6 GiB cap;
host available memory stayed at or above 21,109,440,512 bytes. There were 323
soft-limit reclaim events, no hard-limit or OOM events, and no host OOM kill.
Normal/workspace images were restored byte-identically; no VM was left running.
VirtualBox was not attempted in this slice, and Threshold is unchanged. The
[Ny Object Log](../Development/2026-10-03%20Ny%20Object%20Log.md)
records final evidence and hashes. This qualifies one inspected source/object/startup composition,
not a general Ny target, hosted runtime, loader, application-service ABI or
native Antimatter. The native application prototype is described above; full Antimatter
runtime integration and workspace persistence remain separate gates.

## Application profile — preceding kernel increment

Proposed
[ADR 0048](../../docs/adr/0048-prove-bounded-application-cpu-copy-and-fault-profile.md)
adds the separate `application_profile_test` feature over retained launch and
dispatch ownership. Its CPU session eagerly captures user x87/SSE2 state into
aligned supervisor-only images, loads clean handler state, and restores the
selected task before every return. It admits an explicit Intel baseline or
AMD/Hygon with unconditional x87 error-pointer save/restore, excludes enabled
XSAVE/TLS and unowned fast syscall entries, and restores the original kernel
FP state and control registers before a verified storage wipe.

Copies are limited to 4096 bytes and bind the exact live current task and circle
generations, selected root/stack, mapping permissions, and retained page-table
storage. Full preflight precedes any copying; returned rejections preserve the
destination. Selected CPL3 #DE/#UD/#GP/#PF faults are classified separately from
returnable instruction/stack bounds. A runnable peer continues; when the last
current task faults, common exception bookkeeping is closed and the retained
bootstrap continuation resumes before resources are reclaimed.

The fixture completed four sequential pairs (eight tasks): 64 preemptions and
80 timer ticks per pair, totaling 256 preemptions and 320 ticks per VM. It
checks clean initial state, distinct XMM/x87/MXCSR state, 20 successful and
40 rejected copies, and terminal fault cases with an invalid RIP or RSP.
All 28 final targets passed in 1,153.82 seconds of summed target time on 265
unchanged frozen non-documentation files. The fresh full `make check` took
227.05 seconds and passed 440 default, 483 all-feature, and 449 application-feature
kernel tests plus 106 xtask tests. The new profile passed at 96 MiB and 320 MiB
under TCG's Nehalem `GenuineIntel` baseline and KVM's actual `AuthenticAMD` host
profile. Each run accepted 16 records and restored 24,146/81,490 free frames;
the eight feature events differed only in `cpu_vendor`. Normal and workspace
images were restored and remain byte-identical to the preceding checkpoint.

An initial full check failed on a redundant-return lint in the new host harness.
That single host file was corrected before a fresh complete 28-target matrix;
no earlier partial matrix is counted. The final matrix peaked at 1.984 GiB of
scope memory, with at least 21.6004 GiB host memory available and no soft-limit
reclaims, hard-limit events or OOM events. Across all phases, peak scope use was
4.002 GiB; the failed initial check recorded 501 soft-limit reclaims, with no
OOM kills in any phase. No new VirtualBox proof was run while host KVM was in use.
The
[Application Profile Log](../Development/2026-10-03%20Application%20Profile%20Log.md)
records the completed runs, source freeze, terminal fixture windows and limits.

This is an internal profile, not complete untrusted-program admission. TLS,
AVX/XSAVE, #MF/#XM recovery, a general loader, stable application ABI, permanent
kernel placement and global W^X remain outside its claims. That checkpoint
selected the Ny object/startup qualification completed by ADR 0049 above.

## Owned sustained dispatch — preceding kernel increment

Proposed
[ADR 0047](../../docs/adr/0047-prove-sustained-dispatch-over-retained-launches.md)
retains two prepared-launch owners, two 16 KiB supervisor high-alias privilege
stacks, and activation/cleanup progress through sustained user execution.
Together they own 36 frames (18 data, 18 tables) and ten active/bound leases.
Preparation still publishes no task; explicit commit retains each ready task.

The new scheduler withdrawal accepts only an exact ready, queued initial
context. It rejects any global current task or reservation, stale/wrong
identities, other contexts/states, and corrupt attachments before mutation.
It preserves queue order and sequence cursors, leaving the ready task for the
launch owner's cancellation. A partial pair admission and two privilege-stack
clear failures prove retained rollback and cleanup retries. Clear/release
failure is retryable; failed mapping revalidation remains retained fail-closed,
without a recovery guarantee on repeated `finish`.

Both users complete 64 real PIT preemptions, 32 each, with independent progress,
intact canaries, and nested user RSP at stack top minus 24 bytes. Task 0's exact
armed UD2 faults only that task; task 1 continues for 16 ticks without
artificial redispatch, then exits. Both launch/stack owners remain retained
until both tasks are terminal. Teardown restores bootstrap CR3, TSS.RSP0,
CR0.TS, and the kernel continuation before reclaiming storage and the alias.

Six scheduler-withdrawal tests, four task-withdrawal tests, six strict verifier
groups, and freestanding feature Clippy passed. TCG/KVM runs at 96 MiB and
320 MiB each passed 15 records (seven identical feature events), restoring
exactly 24,150 and 81,494 free frames. All 26 targets qualified in 1,491.42
seconds of selected target time across phases: 14 unchanged earlier native
passes plus 12 final-phase targets, including a fresh full host check.
The final check passed 420 default, 463 all-feature, and 429 owned-feature
kernel tests plus 100 xtask tests in 300.59 seconds. The two new capture tests
also passed their focused run.

A narrow host display-capture correction separates the phases; the initial
transcript rejection's cause remains unconfirmed because the failed artifact
was deleted. An intermediate positive-test mismatch was fixed and revalidated.
All 258 final frozen files remained unchanged, and kernel/other source files
were unchanged between phases. Normal and workspace images match both recovery
and the prepared-launch checkpoint. Scope peak was 4.002 GiB under a 6 GiB cap;
host available memory stayed at or above 10.697 GiB. No scope or host OOM event
was observed. The existing KVM workspace remains active. The
[Owned Sustained Dispatch Log](../Development/2026-10-03%20Owned%20Sustained%20Dispatch%20Log.md)
records the complete evidence, image hashes, and diagnostic limits.
`make owned-dispatcher-smoke` runs both RAM sizes using the separate proof image.
This bounded GPR-only composition supplies no stable ABI, general loader,
FPU/SIMD/TLS ownership, or complete application CPU/copy/fault profile.

## Retained prepared launch — preceding kernel increment

Proposed
[ADR 0046](../../docs/adr/0046-prove-retained-prepared-launch-ownership.md)
adds `prepared_launch` around the existing three-region GPR-only mapping,
entry, circle, and task lifetime. Preparation verifies the admitted geometry,
builds/populates and reads back the payload/mappings, and attaches circle
resources without creating a task. Explicit commit publishes the ready task
only after preparation succeeds. The load plan remains inert; synthetic
fixture identity metadata is supplied separately.

`task_table::cancel_unstarted` accepts only a ready task without an attached
context and makes it terminal before cleanup. The launch owner also handles
partial preparation and terminal execution. It rejects running tasks and
unexpected attachments, records each release/detachment before continuing,
and retains unreleased leases for retry after a mapping-clear failure. Existing
task/circle tombstone-clearing failures remain terminal, with poisoned retained
provenance. Root registrations and continuations must be withdrawn before
ownership is reclaimed.

The native memory proof now covers thirteen failure/cancellation cases: four
allocation-clear failures, table-write and payload failures, circle exhaustion,
cancellation before task publication, and five cleanup-clear failures followed
by successful retries after cancellation of unstarted tasks. It then prepares
and commits the original two users: B produces its reports and contained fault,
then A continues through its reports and exit. The strict transcript preserves
the preceding evidence and adds `kernel.memory_test.launch.verified`.

Four focused task-transition tests, eight owner tests, seven memory-verifier
tests, freestanding Clippy, and 96-MiB/320-MiB TCG/KVM memory proofs passed.
All 24 regression targets passed in 1,059.31 seconds of combined target time,
with 254 frozen non-documentation files unchanged. The memory proofs restored
24,148/81,492 free frames, and normal/workspace images match recovery. Peak
scope memory was 4.00 GiB under a 6 GiB cap; no OOM event occurred.
The [Prepared Launch Ownership Log](../Development/2026-10-03%20Prepared%20Launch%20Ownership%20Log.md)
records the checkpoint's evidence and resource measurements.

ADR 0047 subsequently composed retained launch and sustained dispatch;
ADR 0048 added the internal CPU/copy/fault profile, and ADR 0049 qualified one
Ny object/startup composition above. Surface/input and workspace services stay open.
The ADR 0046 bounded implementation
does not establish Antimatter execution, an artifact ABI, arbitrary user-copy,
TLS/FPU ownership, permanent kernel placement, or global W^X.

## Development environment — 2026-10-03 recovery

The replacement-machine checkout is
an external validation directory (not distributed). The pinned Rust toolchain and
local Eidolon/Ny SDK are restored. All 24 fresh validation targets passed:
21 TCG targets plus KVM memory, dispatcher, and workspace repeats. Corrections
are limited to QEMU proof shutdown policy and two exact saved-RF fixture
predicates; all 370 frozen files remained unchanged through validation.
The accelerated repeats used two build jobs and an 8 GiB scope limit; peak
scope use was 1.54 GiB with no recorded OOM event. A KVM workspace launch also
reached service/frame readiness under a separate 2 GiB limit.
The [Development Environment Recovery Log](../Development/2026-10-03%20Development%20Environment%20Recovery%20Log.md)
records setup, evidence, memory measurements, interactive commands, and the
prepared-launch owner selected at recovery as the next kernel slice. The
historical kernel milestones and their validation evidence below remain
unchanged.

## Owned user execution — previous kernel increment

Proposed [ADR 0044](../../docs/adr/0044-prove-owned-low-address-user-execution.md)
connects the retained low-address mapping owner to two real GPR-only ring-3
tasks and two circle identities. Each root owns RX code at 4 MiB, two RW+NX data
pages, and two guarded RW+NX stack pages. The pure entry plan admits declared
initialized executable bytes and distinguishes initial 16-byte stack alignment
from valid 8-byte-aligned resumed stack pointers.

B executes first, resumes two exact private UD2 reports at different noninitial
stack pointers, and faults on an exact supervisor kernel-text read. A then
resumes its two reports and exits. The proof checks independent private data,
peer progress, preserved TSS.RSP0, restored CR0.TS, wiped privilege stack,
cleared continuations, and retired task/circle generations. Bootstrap restoration
and root withdrawal precede reclamation of all 28 owned frames and eight leases.
The shared exception snapshot reads actual long-mode hardware RSP/SS for CPL0,
CPL3, and IST frames, correcting ordinary diagnostic consumers as well.

All 24 validation targets passed in 1,346.9 seconds of combined target time.
TCG and KVM memory proofs passed at 96 MiB and 320 MiB with 23 structured records
and exact restoration of 24,150/81,494 free frames. All 240 frozen source files
remained unchanged; initial/TCG/KVM memory images match. Focused checks passed
107 x86 tests, eight memory-transcript tests, and freestanding Clippy. Full
suites passed 398 default kernel, 441 all-feature kernel, and 92 xtask tests.
The [Owned User Execution Log](../Development/2026-09-27%20Owned%20User%20Execution%20Log.md)
records final evidence. Normal, dispatcher, and workspace image hashes changed
with the shared snapshot correction; direct feature exclusion and both TCG/KVM
dispatcher/workspace regressions passed. This fixture defines no syscall
ABI, general loader, FPU/TLS ownership, asynchronous dispatch for these roots,
permanent placement, or global W^X. That checkpoint selected retained launch
preparation, explicit commit, retryable cancellation, and per-failure restoration
as its next slice; ADR 0046 implemented that bounded follow-up, ADR 0047 added
sustained dispatch, and ADR 0048 added the application profile. The current
ADR 0049 Ny object qualification is recorded above.

## Resident mapping activation — previous memory increment

Proposed [ADR 0043](../../docs/adr/0043-prove-resident-mapping-activation.md)
adds explicit 4 KiB supervisor mappings for kernel text, constants, data/BSS
and the complete Threshold stack, with absent gaps below 2 MiB. Two owned
roots combine these mappings with the existing four-region user fixture and
the borrowed supervisor RW+NX high alias. The separate activation session
retains both owners through A → B → A → bootstrap; it never grants activation
from a user-only readiness flag.

The native proof checks distinct values at identical low user addresses,
a physical 4 MiB marker through the high alias while virtual 4 MiB maps another
frame, and six exact CPL0 faults: two read-only stores, two guard reads and two
NX instruction fetches. Hardware A/D additions are bounded by entry type.
All 35 temporary frames and 11 leases must be reclaimed after restoration.
The [activation log](../Development/2026-09-26%20Resident%20Mapping%20Activation%20Log.md)
records final evidence. All 24 regression targets passed in 1,111.9 seconds;
94 focused x86 tests, eight memory transcript tests, and freestanding Clippy
passed. Full kernel suites passed 385 default and 428 all-feature tests; xtask
passed 92. TCG and KVM both restored 24,152/81,496 free frames at 96/320 MiB.
All 236 frozen source files were unchanged, initial/TCG/KVM memory images match,
and normal/dispatcher/workspace images match the preceding checkpoint.
Independent reviews found no remaining issue in the new bounded proof.

This is a bounded BSP bridge, with no ring-3 task launch,
permanent high-half kernel layout, global W^X or general fault containment.

## Owned mapping construction — previous memory increment

Proposed [ADR 0042](../../docs/adr/0042-prove-inactive-owned-user-mapping-construction.md)
adds a retained owner for real inactive user page tables. Four native fixture
regions use seven data frames and nine table frames, allocated as five genuine
leases. A temporary high-half supervisor RW+NX alias of Threshold's validated
tables supplies privileged access while the original root remains selected.
The stage retains every lease after construction or cleanup failure; complete
table withdrawal precedes data release and retries restore exact accounting.

The 81-test focused x86 run includes eight new ownership tests and three new
physical-access tests. Eight memory transcript tests include three new strict
rejection groups. Freestanding Clippy passed. The initial native proof passed
at 96 MiB and 320 MiB, including five allocation, one population, and six cleanup
failures. All 24 regression targets passed, including the same memory proof at
both RAM sizes under KVM. All 233 frozen source/configuration files remained
unchanged; initial, TCG, and KVM memory images match. Normal, dispatcher, and
workspace images are byte-identical to the preceding checkpoint.
Independent reviews found no remaining issues.
The [construction log](../Development/2026-09-26%20Owned%20Mapping%20Construction%20Log.md)
records evidence and limits. These user-only roots remain inactive. The resident
increment above supplies a separate bounded activation profile; the retained
physical alias still permits writes to executable physical storage.

## User mapping plans — previous memory increment

Proposed [ADR 0041](../../docs/adr/0041-plan-bounded-user-mappings-and-physical-access.md)
adds kernel-private, allocation-free and unsafe-free memory planning. Up to
eight user regions and eight exclusions are checked for complete range validity,
overlap, supervisor/guard collisions, and W^X. Separate data and table budgets
count one root plus exact unique user-tree branches, bounded to 32 frames.
Executable membership preserves gaps and per-region rights. A separate typed
physical-window descriptor checks bidirectional whole-range offset arithmetic
without constructing a pointer or selecting a permanent kernel layout.

All 19 new tests passed within a 70-test x86 run, and warning-denied freestanding
Clippy passed. Independent review found no further issues. All 23 kernel/desktop
regression targets passed against 230 unchanged frozen source/configuration files;
normal, dispatcher, and workspace images are byte-identical to the preceding
checkpoint. The [mapping log](../Development/2026-09-26%20User%20Mapping%20Plan%20Log.md)
records evidence. The live mapper remains unchanged: no new root is allocated,
no low user mapping is activated, and no allocator rollback claim is made.
Lease-backed construction and a valid privileged physical-access path under
each root remain the next native gates.

## Passwordless identity — proposed direction

The user selected identities/entities for humans and agents without native
username/password authentication, with biometrics as a future way to verify a
human locally. Proposed
[ADR 0040](../../docs/adr/0040-separate-entity-identity-authentication-and-authority.md)
keeps a stable entity, its replaceable authenticators, live execution subjects,
and scoped authority distinct. Biometrics authorize use of a protected key;
they are not the entity identifier, a derived encryption key, or data supplied
to an AI context. Administration remains explicit maintenance and recovery
operations with separately granted capabilities.

Holder-bound delegation, retained session state, and explicit mediation remain
requirements. Enrollment, durable recovery, and a deployed identity broker are
open. This increment documents the direction; it changes no
kernel code, authenticator, token format, or native ABI. See the
[identity design](../../docs/architecture/identity-and-authority.md) and
[Passwordless Identity Log](../Development/2026-09-26%20Passwordless%20Identity%20Log.md).
At that checkpoint, general launch ownership, checked user-copy, and fault
policy remained gates before the native launch contract and Linux ELF work.
ADRs 0046–0048 above subsequently add retained launch ownership and the bounded
internal application profile; broader application admission remains open.

## Sustained native dispatch — validated kernel increment

The user selected kernel foundations before a modular native execution boundary,
Linux ELF first, and macOS afterward. Proposed
[ADR 0039](../../docs/adr/0039-prove-bounded-sustained-native-dispatch.md) adds a
separate removable sustained-dispatch proof: two private GPR-only tasks, 64 real
PIT switches, one exact known fault, and 16 further timer ticks with measurable
survivor progress. The sole-runnable path must not manufacture new dispatch
sequence values. Final TCG and KVM runs both passed the exact 14-record
transcript, 80 total PIT ticks, 66 dispatches, and restoration of 65,112 free
frames. All 23 validation targets passed in 1,071.6 seconds, with 228 frozen
source/configuration files unchanged. Both workspace runs retained 21 keyboard,
410 pointer/Appearance, and 23 graphics frame comparisons; the workspace image
matches the prior graphics checkpoint exactly. A separate isolated VirtualBox
attempt was blocked by KVM ownership of AMD-V and cleaned up without changing
existing VMs or host settings. ADR 0039 remains Proposed.

Source audit corrects the native-format assumption: experimental native rite
prefixes exist, but the canonical semantic invocation encoding, general
versioned launch contract, and Reliquary artifact format are not finished.
At that checkpoint no runtime Linux ELF loader or foreign syscall personality
existed. The [execution-readiness audit](../../docs/architecture/execution-readiness.md)
records the implemented boundaries and remaining address-space, mapping,
CPU-state, user-copy, fault, and launch prerequisites. At that checkpoint, the
ordered follow-up was native launch contract, Reliquary execution framing,
Linux ELF admission and execution personality, then an independent Mach-O/Darwin
veil. ADR 0045 subsequently selected native Antimatter before foreign execution,
as recorded in the current direction above. The
[Sustained Native Dispatch Log](../Development/2026-09-26%20Sustained%20Native%20Dispatch%20Log.md)
records the proof and regression outcomes. This slice does not mark all
kernel foundations complete or freeze a stable ABI.

## Native window content surfaces — validated

The current request adds graphics and rendering inside native windows, preparing
for Linux-first TRI-ABI work. Proposed
[ADR 0038](../../docs/adr/0038-add-native-window-content-surfaces.md) introduces
portable validated immutable RGB8 surfaces, caller-owned raster drawing, exact
per-window content metadata, and a distinct granted presentation owner. The
compositor validates all retained bindings before presentation and clips scaled
content to the shared client viewport. Graphics Lab uses **F2** to open/restore
and **F3**, while focused, to switch between two scenes drawn once at startup
within the existing trusted workspace vessel.

This is a native graphics contract and fixed demo. Independent application
processes, shared-memory transport, foreign executable/syscall/runtime support,
Wayland/X11, private-content policy, and a general accessible content tree remain
open. Linux is the first intended compatibility target; macOS uses Darwin/XNU
with Mach/BSD components and remains an independent veil under ADR 0008.
Component suites and `make check` passed. A maximized-chart run acknowledged
200 moves at 100 Hz, published 165 frames, and matched the final 786,432-pixel
capture exactly. Isolated KVM input-to-frame medians range from 8.77 ms on the
default desktop to 10.27 ms with the chart maximized. A separate **Anomalous
Workspace - Graphics Lab** session was verified with KVM active and later
exited; its helper cleaned up the private disk. `make workspace-run` starts a
fresh session from the reusable development image.
All 20 validation targets and an additional complete KVM workspace smoke
passed on unchanged final source. Each workspace run verified 23 graphics,
21 keyboard, and 410 pointer/Appearance frames, all five containment paths,
complete resource restoration, and working shell fallback. The
[Window Content Surfaces Log](../Development/2026-09-26%20Window%20Content%20Surfaces%20Log.md)
records this increment separately from prior validation. Filesystem and file-app
work remain later user-directed slices.

## Mouse responsiveness follow-up — validated

The preceding request fixed slow mouse interaction before choosing the next
foundation or application slice. On the final image, 24-sample native probes
measure median command-to-complete-frame times of 8.9 ms for the default
desktop and 11.2 ms with Appearance under KVM; TCG measures 51.1/68.8 ms.
The original image measured 163.5/245.1 ms under TCG and 19.5/29.4 ms under
KVM. These are host monitor-input-to-serial-frame measurements, not physical
pointer-to-screen latency.

Five sustained runs inject 200 moves at 100 Hz. Final KVM positive/sweep runs
finish within 0.0/2.8 ms after the full two-second interval; the last command
precedes that endpoint by about 10 ms. Final TCG tails are 98.3/123.9 ms,
compared with 3434.4/3353.0 ms before batching. All five complete native captures
match expected pixels, revision, and cursor. An unclamped 200 one-pixel TCG
run ends exactly at (712, 384) from (512, 384), preserving cumulative motion.
Host load remains uncontrolled and affects these observations.

The compositor now skips redundant opaque/interior and off-row work. Exact
mailbox copies, framebuffer writes/readback, and stage clearing use aligned
word paths with bounded byte fallbacks. Graphical Linux QEMU defaults to KVM
with TCG fallback; `ANOMALOUS_QEMU_ACCEL=auto|kvm|tcg` selects the launch policy.
Proof runners keep their existing backend. Proposed
[ADR 0037](../../docs/adr/0037-batch-ready-desktop-pointer-motion.md) adds at most
16 already-ready continuous same-button packets per update, validating each
raw sequence and applying each viewport clamp in order. Press/release/chord
changes, loss, keys, and source changes stay separate, with at most one
untouched lookahead record. Capture retains its original anchor, and no delay
collects input. One canonical transaction still needs a complete 768-row
acknowledgement; full verified clearing remains in force. The initial matrix
was stopped when batching changed the code; the fresh complete 20-target pass
and additional KVM workspace smoke both passed. Each native smoke verified
21 keyboard and 410 pointer/Appearance frames, both sessions, all five
containment paths, full resource restoration, and working shell fallback.
The final image is running as **Anomalous Workspace - Mouse Integration**, with native readiness, a
complete first frame, and active KVM verified. Existing sessions remain intact.
See the [Mouse Responsiveness Log](../Development/2026-09-26%20Mouse%20Responsiveness%20Log.md).

The user has now selected native window content as the next slice above.
The completed validation statements below describe earlier increments.

## Desktop click lifecycle checkpoint

Proposed [ADR 0036](../../docs/adr/0036-activate-desktop-controls-on-pointer-release.md)
adds release-to-activate buttons, cancel-by-moving-away, and an explicit public
armed target. Feedback cannot follow a held button into an unrelated control.
Keyboard input, chords, source loss, and scene revisions cancel the gesture;
new sources wait for an all-buttons-up observation. All 20 validation targets
passed. Final suites verified 37 model tests plus 8 compile-fail docs,
38 compositor tests, and 34 input adapter tests. Native smoke verified 21
keyboard frames and 410 pointer/Appearance frames, including 73 new
release-gesture frames, with complete resource restoration and shell fallback.
A fresh **Anomalous Workspace - Interaction Checkpoint** session uses the final
inspected image. This increment changes no kernel source or native input ABI.
See the [Desktop Click Lifecycle Log](../Development/2026-09-26%20Desktop%20Click%20Lifecycle%20Log.md).

At that checkpoint, a future foundation candidate was a separate bounded
two-task dispatcher proof with sustained timer preemption, continued peer
progress after an exact task-local fault, and complete reclamation. It was
unimplemented then; proposed
[ADR 0039](../../docs/adr/0039-prove-bounded-sustained-native-dispatch.md) now
implements the bounded experiment, with current validation recorded at the top
of this note. The workspace still starts an unscheduled task, and neither the
accepted scheduler proof nor the new experiment is a persistent multi-vessel
runtime. General admission, wait/wake, service loading, and restart policy
remain broader work. Desktop app processes, private text, persistent settings,
and external-agent IPC remain separate later boundaries.
All completed passes below describe earlier work.

## Built-in Appearance app increment

Proposed [ADR 0035](../../docs/adr/0035-add-a-built-in-appearance-app-with-shared-choice-state.md)
adds a singleton Appearance app inside the existing native workspace owner.
The header button or **P** opens it; explicit mouse choices and
**F6**/**Shift+F6**, **Enter**/**Space** use canonical selection and preferences.
**Tab** still traverses windows. Sixteen options cover themes, density, text
size, cursor shape, frame corners, depth, reduced motion, and reset. Small
windows expose a maximize prompt instead of hidden controls, and incompatible
retained overrides disable their theme choices. Settings remain session-local;
there is no new application process, private text, or external agent IPC.

Bounded geometry, capacity, theme, and layout rejections now preserve the
native owner; unexpected source, authority, transaction, and presentation
failures still withdraw it. Final suites passed 31 model tests plus 8
compile-fail documentation tests, 18 theme tests, 23 adapter tests, and 37
compositor tests. All 20 validation targets passed, including 21 exact native
keyboard frames and 337 pointer/Appearance frames across two sessions with
capacity rejection, complete resource restoration, and working shell fallback.
The final `make check` was repeated after the corner-override review fix.
A fresh **Anomalous Workspace - Appearance (Final)** graphical session uses
the inspected final image. See the
[Appearance App Log](../Development/2026-09-26%20Appearance%20App%20Log.md).
The completed full passes below describe earlier increments. A future native
console needs move-only private text owners and disclosure-aware views before
admitting sensitive input; it must not place private text in the public `Copy`
workspace snapshot.

## Clay-inspired frame asset increment

Proposed [ADR 0034](../../docs/adr/0034-build-shared-native-frame-assets-and-canonical-control-states.md)
adds original compiled vector window controls, Clay-inspired Dark and interpreted
Light presets, validated decoration/frame components, rounded native frames,
blended shadows, and canonical pointer-button state. The generated SVG sheets
are reviewable design references; the native compositor still owns bitmap text
and exact pixels. No external asset loader, new font backend, or application
runtime is introduced.

Focused native smoke passed 21 complete keyboard frames and 124 pointer frames,
two fresh desktop sessions, and all five contained termination paths with full
resource restoration and working shell fallback. Component tests and exporter
checks passed. All 20 validation targets passed in 867.0 seconds; the final host
compositor suite passed 31 tests after the last two review fixes. A fresh
**Anomalous Workspace - Clay Frames (Validated)** QEMU session reached native
service readiness and its complete 768-row first frame with an empty stderr
log. The [Clay Frame Assets Log](../Development/2026-09-26%20Clay%20Frame%20Assets%20Log.md)
records its evidence; the full passes below belong to the earlier increments.
VirtualBox was not rerun for these frame changes; the earlier host KVM conflict
remains historical evidence rather than a new runtime result.

## Native themes and pointer shell increment

Proposed ADR 0033 extends the native workspace with the first official Light
and Dark themes, independent validated appearance components, a task panel,
pointer focus/drag/resize, minimize/maximize/restore, and a repeatable `desktop`
shell command. Human and machine commands share bounded atomic batches and one
canonical cursor/window state. Login screens, sounds, font/theme asset packages,
durable profiles, animations, and real application surfaces remain open.
The [Native Themes and Pointer Log](../Development/2026-09-26%20Native%20Themes%20and%20Pointer%20Log.md)
records implementation and current validation evidence. All 20 validation targets
passed, including 21 complete keyboard frames, 124 pointer frames, two fresh
native sessions, all five contained termination paths, and normal-image
exclusion. A new graphical QEMU opened with working pointer input as
**Anomalous Workspace - Light and Dark**. VirtualBox remained blocked by the
host AMD-V/KVM conflict. The previous validated keyboard-only increment is
recorded below as its historical baseline.

## Native window workspace implemented and validated

Proposed [ADR 0032](../../docs/adr/0032-start-a-native-window-workspace-and-compositor-service.md)
starts the next desktop increment through `make workspace-run`. A separate
persistent ring-3 vessel owns canonical window state, keyboard policy, and CPU
composition. The portable `anomalous-desktop-workspace` model and
`anomalous-desktop-compositor` now run natively. `workspace-smoke` passed 21
exact full public frames through real keyboard input. Controlled fault,
watchdog, malformed mailbox, malformed saved flags, and orderly exit all
restored resources and left a working bootstrap shell. The full 20-target
validation matrix passed, including fresh desktop/workspace smoke and final
normal-image exclusion. Current evidence belongs in the
[Native Workspace Development Log](../Development/2026-09-26%20Native%20Workspace%20Development%20Log.md),
not in the accepted ADR 0025–0029 fixture claims.

The model has four public demo window slots, two workspaces, and four local
client endpoints. It supports creation/close, focus/raise, move/resize,
minimize/restore, workspace membership, floating/horizontal/vertical layout,
and theme, density, depth, and reduced-motion preferences. Floating geometry
survives tiling and focus does not reorder tiles. Human and machine endpoints
submit the same typed commands with explicit grants and revision checks;
preparation also requires observation authority because its ticket exposes the
candidate. The native keyboard adapter is the first live client. There is no
external agent connection, arbitrary app surface, private content, or executable
plugin.

Presentation starts from an unpublished initial candidate and permits one
pending successor. Once presentation begins, observations and actions remain
unavailable until the complete frame is acknowledged and its revision is
published. The native path copies full-width rows into a private kernel stage;
user space never receives the framebuffer. The first owner-loss policy is
withdrawal and bootstrap-shell fallback with resource reclamation, discarding
the windows. It does not implement surviving-owner recovery from ADR 0030.

The graphical workspace launched on a private disk and supported hands-on
testing while the full validation matrix ran. The interactive session presented
five frames before an orderly Esc withdrawal. At that checkpoint, QEMU remained
open at the bootstrap-shell prompt and that session was preserved. The separate
`workspace-dev.img` and per-run copies protect live VMs across rebuilds. VirtualBox startup was attempted but
blocked by the host KVM/AMD-V conflict (`VERR_SVM_IN_USE`); no host settings
were changed. That increment was keyboard-driven; the newer ADR 0033 adds
pointer input. Text input,
real apps, semantic IPC, persistent settings, restart recovery, and GPU work
remain the next production boundaries. The earlier `desktop-run` launcher below
remains available as its own regression path.

## Interactive development desktop

Proposed ADR 0031 adds an opt-in development launcher: `make desktop-run` boots
the public scene through the isolated native renderer, Enter opens the
bootstrap shell, and `desktop` restores the launcher. The interactive QEMU
smoke, graphical launch, full `make check`, all required QEMU regressions,
additional display/renderer smoke tests, and final normal-image exclusion
passed. The live launcher uses a private disk copy and releases the shared
build lock, allowing development alongside it. VirtualBox startup was attempted
but blocked by the host KVM/AMD-V conflict (`VERR_SVM_IN_USE`). See the [Interactive Desktop Development Log](../Development/2026-09-26%20Interactive%20Desktop%20Development%20Log.md).
This is a temporary bootstrap loop, not the deployed user-space desktop service.

## Verified today

Work resumed on 2026-09-26 with proposed ADR 0030's host presentation-recovery
coordinator. It is the first prerequisite for the native-service boundary,
not a deployed service. The development log records implementation and fresh
validation separately from the accepted native evidence below:
[Presentation Recovery Coordination Log](../Development/2026-09-26%20Presentation%20Recovery%20Coordination%20Log.md).

The implemented `anomalous-desktop-recovery` wrapper passed 5 unit tests,
12 integration tests and 37 ownership/privacy compile-fail tests, plus the
complete `make check` and serial QEMU regression matrix and final normal-image
exclusion. It withholds semantic views across copy, publication and final
acknowledgement; one authorized fresh presenter must redraw the retained
successor before reads reopen. Recovery preserves an already committed revision
without publishing it twice. The canonical coordinator must survive; native
process authentication, service-loss recovery and deployment remain open.

Milestone 1 kernel foundations are in progress atop the demonstrated
milestone-0 seed. These are observed capabilities, not roadmap claims:

- Threshold owns the BIOS boot path; GRUB is not involved.
- Its manifest independently describes and checks the second stage and kernel.
- Threshold reaches x86-64 long mode and hands a versioned `boot_omen` to the
  allocation-free Rust kernel.
- The kernel validates the handoff, reports memory and display state over COM1,
  and draws the initial neumorphic framebuffer surface.
- The kernel loads its own GDT and TSS, provides fixed privilege, double-fault,
  and NMI stacks, and installs a fatal handler for all 32 CPU exception vectors.
- A bounded QEMU fault image proves that only the exact armed invalid-opcode
  vector, address, and selector are accepted as test success.
- The bounded QEMU smoke test reaches `boot.ready`.
- The VirtualBox VM `Anomalous Dev [Threshold]` reached `boot.ready` with a
  1024 by 768, 24-bit framebuffer on 2026-09-08.
- Normal images continue into a bounded bootstrap command loop that drains a
  private interrupt-fed PS/2 channel through a separate provisional US ASCII
  adapter, with a fixed-capacity editor, framebuffer or VGA-text output, and
  five read-only commands.
- Host tests cover private PS/2 decoding, typed queue/loss behavior, layout,
  editing, and command classification. The fresh QEMU shell smoke test corrects
  `versiom` with Backspace plus `n`, then verifies privacy-safe `echo` and
  unknown dispatch while proving its privacy marker is absent from serial
  telemetry.
- An earlier VirtualBox shell checkpoint accepted all five built-ins, an edited
  command, private `echo` content, and unknown input; it remained running at the
  next prompt while COM1 exposed only static command kinds and structured
  results.
- The diagnostic-fault refresh of that VM emitted `kernel.fault_path.ready`,
  accepted an injected `version` command, emitted the next prompt, and remained
  running.
- The feature-only first-circle image completes three ring-3 generations under
  both QEMU and VirtualBox: exact supervisor-read error `0x5`, exact NX-stack
  fetch error `0x15`, verified teardown after each generation, and a clean
  terminal generation.
- The strict QEMU validator derives exact fault addresses from the built ELF,
  checks the complete structured lifecycle, and proves the prototype is absent
  from the normal image. At that checkpoint, the dedicated VirtualBox VM was
  restored to the normal bootstrap shell and observed running at its prompt.
- Threshold now attests only a complete version-2 E820 enumeration. It requires
  VBE 2.0 or newer, selects version-appropriate legacy or linear framebuffer
  fields, and validates geometry, extent, channel separation, and occupied-
  memory separation before graphics activation or publication. The kernel
  independently copies and validates both handoff descriptions, including RGB
  bit-depth bounds and non-overlap, and fails closed with attributable machine
  and human diagnostics.
- The accepted bounded memory manager normalizes whole frames below four GiB,
  reserves low, kernel, and framebuffer memory, and grants allocator- and
  generation-bound leases whose complete extents are cleared and verified.
- Feature images at 96 MiB and 320 MiB under QEMU and 512 MiB under VirtualBox
  proved two allocator-backed address spaces, A-to-B-to-A marker separation,
  explicit rejection of overlap, duplicate binding, and active-root destruction,
  CR3 withdrawal, zeroed same-frame reuse, and exact free-count restoration.
- ADR 0013's BSP-only `legacy_pc` backend installs distinct returning IRQ entry,
  owns PIC/PIT/8042 initialization, advances immutable boot timebase generation
  1, and publishes fixed typed physical transitions with exact loss before later
  delivery. Loss contains IRQ1 until bounded device reinitialization advances
  the source generation and restarts its sequence. NMI remains masked.
- QEMU and VirtualBox each reported initial PIC masks 255/255 and active masks
  252/255, eight real PIT interrupts, and 24 ordered private generation-1
  keyboard transitions at sequences 0 through 23. QEMU advanced tick 0 to 8;
  VirtualBox advanced tick 1 to 9 and reported 130,674 free feature-image frames.
  Both preserved return state, reported exact generation-1 loss at sequence 88
  with count 1, contained IRQ1 at master mask 254, completed bounded device
  reinitialization, rearmed generation 2 at master mask 252, delivered
  generation-2 sequences 0 through 1, restored final masks 255/255, and exposed
  no private probe, error, or fatal record in COM1.
- ADR 0014's fixed feature-only proof binds allocator-backed, one-shot ring-3
  tasks to kernel-owned circle, task, endpoint, sigil, local-binding, and
  lineage records. Task-local references resolve through composite authorized
  queue calls, accepted messages are copied and stamped from current kernel task
  state, and the two synthetic producer roles use byte-identical code without
  turning their labels into authentication.
- The strict QEMU control proof accepted exactly 28 structured records and
  restored its initial 65,114 free frames. The dedicated VirtualBox VM
  `Anomalous Dev [Threshold]` reproduced the exact 28-record lifecycle and
  restored its initial 130,667 free frames. Both runs completed without an
  error or fatal record.
- Independent final review made raw queue mutation module-private, required
  composite authorized calls and complete occupied-record validation, made
  sigil allocators fail closed, and derived receiver and final accounting from
  the lifecycle before the final emulator evidence was generated.
- ADR 0015's feature-only scheduler admits two allocator-backed tasks with
  private roots, distinct privilege stacks, and exact saved contexts. Two real
  PIT IRQ0 deliveries preempt them once each; one exact resumed-task `#UD`
  faults only that task, and the peer then resumes and completes.
- The strict QEMU scheduler proof accepted exactly 25 records at PIT ticks 1
  and 2, restored all 65,112 initial free frames, and restored every task,
  circle, context, queue, lease, bound-lease, and mapping count. The exact
  512 MiB VirtualBox VM reproduced the lifecycle at ticks 2 and 3 and restored
  all 130,665 initial free frames.
- Independent scheduler reviews hardened complete frame/snapshot validation,
  static unexpected-exception diagnostics, privilege-stack and address-space
  ownership, normal-image exclusion, malformed/mutated transcript rejection,
  and successful-path scrubbing. Partial setup or teardown remains terminal and
  quarantines any ownership not conclusively withdrawn.
- ADR 0018's accepted feature-only proof preserves one user-space supervisor
  while one vessel generation faults. The supervisor consumes one kernel-owned
  observation, authorizes one destructive restart at a safe kernel-stack
  checkpoint, and observes a cleared replacement generation complete. It
  restores scheduler, supervision, task, circle, mapping, and allocator
  accounting.
- ADR 0019's accepted feature-only x86-64 rite proof admits one fixed ring-3
  shell-task fixture through DPL-3 vector `0xf3`. Nine ordered calls cover six
  rejections, a 16-byte `console_write` commit, empty `input_read` as
  `would_block`, and terminal `task_exit` without reply copy or `iretq`.
- The strict rite verifier accepted exactly 20 structured records, exposed no
  private console sentinel, restored all 65,116 initial free frames, zeroed
  active/data/table accounting, and proved the gate namespace, helpers, fixture,
  and private marker absent from the normal image.
- For ADR 0019 acceptance, normal-image exclusion rebuilt and inspected the
  ordinary image and proved the rite gate, namespace, helpers, fixture, and
  private marker absent. This slice did not run VirtualBox.
- ADR 0020 now accepts the first composed feature-only shell-vessel skeleton.
  Across one destructive restart, two ring-3 incarnations use fresh task-bound
  rite channels while a persistent supervisor survives one real PIT preemption
  and one exact contained `#UD`.
- The fixed human-source and machine-direct projections each commit the same
  private 17-byte console semantics and observe empty input. Generation 2 exits
  terminally through the rite gate, which copies no reply and selects the
  awakened supervisor rather than returning to the exited vessel.
- The strict composition verifier accepted exactly 38 information records,
  restored all 65,102 initial free frames and zero active object, authority,
  supervision, scheduler, context, mapping, and rite-channel accounting, exposed
  no private fixture data, and proved the entire feature absent from the normal
  image. This acceptance did not run VirtualBox.
- ADR 0021 accepts the host-only `anomalous-session` proof. Its fixed-capacity
  state machine owns bounded private semantic actions, exact FIFO backpressure,
  checked same-lineage session replacement, one consumer, and one
  lost-wake-safe wait without adding a parser, evaluator, authority, kernel
  endpoint, or wire ABI.
- The proof keeps private owners and local receipts move-only, non-formatting,
  and non-hashing; uses explicit disclosure and logical-clear boundaries; and
  defines content-free errors and a privacy-limited observation shape. Origin
  labels remain attribution rather than authentication, observation does not
  authorize emission, and logical clearing is not a secure-erasure claim. The
  embedding host must reserve the complete slot lineage and explicitly close,
  deliver, and acknowledge before destruction.
- At an earlier accepted checkpoint, the ordinary image was restored to the
  dedicated VirtualBox VM with 130,683 free frames and was observed at
  `bootstrap_shell.ready` without an error or fatal record. At the earlier
  interrupt acceptance checkpoint, it also corrected `versiom` with Backspace
  plus `n`, accepted the resulting `version` through IRQ1, and emitted the next
  prompt.
- On 2026-09-13, the ADR 0021 acceptance run passed 8 `anomalous-abi` tests;
  51 `anomalous-session` unit tests and 56 compile-fail documentation tests;
  kernel configurations with 235 default, 240 circle, 235 control, 240 rite, 241
  scheduler, 241 supervision, 246 shell-vessel, 235 memory, and 235 interrupt
  tests; and 35 `xtask` tests. Formatting, script syntax, warning-denied host and
  x86-64 target Clippy, every established QEMU smoke target, and normal-image
  exclusion passed. VirtualBox was not run or required because this host-only
  slice changes no boot or kernel code.
- ADR 0022 accepts the removable x86-64 native session-input bridge. Its
  versioned 16-byte request and 280-byte fixed delivery preserve the physical
  versus semantic input boundary, while one owned pending Rite record and one
  endpoint-gated scheduler reservation bind the exact task, route, wait, and
  readiness across blocking.
- The selected consumer path invalidates the old header, copies the body,
  selects only the reserved consumer, atomically acknowledges and consumes,
  settles the channel, and publishes the reply header last. All 280 injected
  body-copy failures preserved exact endpoint/private-queue, Rite-channel,
  task-table, scheduler, and retained-selection state. Host/native conformance
  covered all 625 four-step traces and the `u64::MAX - 6`/`u64::MAX - 5`
  exhaustion frontier.
- The strict input-wait QEMU proof accepted exactly 19 content-free records,
  including 11 feature-lifecycle records. The full QEMU matrix,
  normal-image exclusion, `make check`, and an explicit all-feature x86-64 ELF
  link passed; independent atomicity, lifecycle, unsafe/privacy, verifier, and
  feature-precedence reviews were clear. This acceptance did not run
  VirtualBox.
- The earlier interrupt/scheduler final review closed fatal serial-boundary,
  strict transcript, empty-IRQ reset, protocol-byte handling, exact PIT-write,
  and complete feature-helper exclusion gaps before that QEMU and VirtualBox
  evidence was regenerated.

- ADR 0023 accepts the separate bounded host authenticated-source and
  focus-routing proof without claiming credential authentication or a deployed
  native router.
- ADR 0024 accepts the first visible output produced by a narrowly granted,
  feature-only ring-3 display vessel. It copies exact versioned RGB8 patches
  through the Rite mailbox to an exclusive kernel framebuffer presenter
  without mapping the framebuffer into ring 3.
- The strict display proof accepted exactly 16 records and 772 calls: three
  rejection probes, 768 full-width commits, and terminal exit. Every rejected
  call preserved the complete canary-backed aperture, teardown restored all
  tracked ownership, and an independent oracle matched all 786,432 pixels in
  the captured 1024x768 frame.
- `make display-demo` leaves that same inspected public fixture visible in a
  QEMU window for end-user review. The pixels remain presentation evidence,
  not an authoritative desktop scene, accessibility tree, focus model, or
  window protocol.
- ADR 0026 accepts the removable supervised semantic-renderer composition. Its
  separately linked freestanding x86-64 load image is 6,469 bytes, occupies two
  image pages, and receives only bounded RX image, RW+NX mailbox, and four-page
  RW+NX stack mappings with `display_update | task_exit` authority.
- Generation 1 survived one real PIT preemption, staged exactly 384 rows,
  reached the exact linked `#UD`, and was fully withdrawn without changing the
  live framebuffer. A fresh generation 2 staged all 768 rows and exited
  terminally before the kernel made one full-frame commit.
- Two protected-source target ELFs were byte-identical. The independent 1024x768
  oracle matched all 786,432 captured pixels, all tracked resources returned
  to their exact initial state, and normal-image exclusion covered the feature,
  private sentinels, and renderer glyph fingerprint.
- The ADR 0026 acceptance run passed `make check` with 329 all-feature kernel
  tests, 56 `xtask` tests, 8 renderer-base tests, and 9 scene-conformance tests,
  plus the full established non-VirtualBox QEMU matrix. VirtualBox was not used
  for this slice.
- ADR 0027 accepts the separate host-only `anomalous-desktop-service` proof.
  It owns ADR 0025's fixed 1024x768, eight-node scene profile, exposes exactly
  one complete committed human/accessibility/machine revision, and retains at
  most one complete private successor through queued, leased, ready, and
  presented phases.
- Strict backpressure returns a competing candidate owner unchanged. One exact
  renderer endpoint may lease an immutable redacted snapshot; one authorized
  renderer replacement preserves that snapshot under a fresh generation and
  second attempt; and renderer readiness alone cannot publish. Only an exact
  receipt minted through the separate trusted presentation control commits the
  prepared semantics and releases producer capacity.
- The service's action/focus facade wraps the scene's move-only local handles,
  exposes only the committed revision, performs no effect, and makes old
  revision handles stale only when publication succeeds. Host evidence covers
  64 valid and 32 invalid initial states, every 64-by-64 valid transition,
  full-slot rejection in every occupied phase, owner-drop non-transitions,
  directed restart and retirement paths, stale and foreign owners, equal public
  IDs under distinct private brands, protected values of unequal lengths,
  static errors, and compile-fail ownership/privacy boundaries.
- ADR 0027 changes no native operation or image. The kernel, `anomalous-abi`,
  Rite layout, ADR 0026 capsule and renderer, framebuffer presentation path,
  boot artifacts, and normal runtime remain unchanged. Its presentation
  receipt is a trusted host assertion, not framebuffer, scanout, or renderer-
  correctness evidence.
- ADR 0028 accepts the separate removable x86-64 native transport proof. One
  explicit 528-byte little-endian redacted snapshot crosses from a narrowly
  granted source into a kernel-owned one-slot copied endpoint and then into a
  separately granted consumer. Exact task, endpoint, surface, scene, revision,
  renderer, and attempt state remains trusted correlation rather than wire
  authority.
- The packaged source flat image is 2,384 bytes in one page and the consumer
  flat image is 5,094 bytes in two pages. Their inspected ELF files are
  1,107,792 and 1,185,928 bytes respectively. The source ELF hash is
  `8efcac631ad19bff57938b5f1aac82d4b3d9df0c2a6009bff05e8f936d6c0046`,
  the 528-byte snapshot hash is
  `97a7e5a8544f2a8a3584beb6df6d1f6052627184df7469c8674545c7fa41871b`,
  and the consumer ELF hash is
  `79076d34284168f8c7b3452e8d7610f6a53cba44b60fa89dc7d001a9cc654f94`.
- The strict QEMU lifecycle completed 11 Rite calls, 9 body copies totaling
  2,176 bytes, one submission, one delivery, one backpressure result, two
  authority denials, two malformed results, two terminal exits, and two kernel
  continuations. It published the consumer reply header last, cleared stale
  reply bytes, retired the source before delivery from the independent endpoint
  copy, consumed the body once, and restored every tracked resource exactly.
- Host fault injection covered every one of the 528 source-copy offsets and
  every one of the 528 destination-copy offsets without premature mutation or
  consumption. Runtime-derived protected probes of different values and
  lengths produced byte-identical bodies and native source payloads; recursive
  artifact and transcript scans found no protected input. The full configured
  host/QEMU validation matrix, dedicated transport smoke, all-feature link and
  precedence checks, warning-denied Clippy, artifact inspection, and normal-
  image exclusion passed. VirtualBox was not run or required.
- ADR 0029 accepts the bounded composition of one live ADR 0027 successor,
  ADR 0028's exact ordinal-61 body, a separately linked ring-3 renderer, one
  supervisor-only 576-frame stage, complete framebuffer verification, an
  independent 786,432-pixel host oracle, and the exact host publication from
  revision/sequence 1/1 to 2/2.
- The strict native lifecycle completed 777 Rite calls, including 775 body
  copies totaling 2,392,160 bytes and exactly 768 ascending full-width row
  commits, before one complete-frame commit. Teardown cleared and released the
  private stage and restored all 65,095 initial free frames before the host
  transaction could publish.
- The ADR 0029 source flat image is 2,384 bytes in one page with SHA-256
  `8bab87d92fa74a43c47321c13743bb371c176169b83cf32bcb9f089a2723f645`;
  the renderer flat image is 10,352 bytes in three pages with SHA-256
  `666c638bd52d76b4883ae0b4bb2a4892fb02fcba0e572440db49ab950e4edc74`.
  The independent host oracle matched every captured RGB pixel before the
  retained private controls were consumed to publish revision 2.
- This evidence covers one successful attempt. Early-exit and partial-stage
  refusal are structural and component evidence rather than a negative QEMU
  run. The declared initial framebuffer is a mutation-oracle baseline, not a
  claimed visual rendering of revision 1. VirtualBox was not run or required.

The seed commit is `ad53279`; the bootstrap-shell checkpoint is `9025a4e`;
the diagnostic-fault checkpoint is `03b852a`; ADR 0011 was proposed at
`603381e`; the first-circle implementation checkpoint is `1d11e3b`; its
accepted hardening checkpoint is `37fb0b7`; ADR 0012 was proposed at `a7696ab`;
the bounded-memory implementation checkpoint is `9d37fd0`; its authority
hardening checkpoint is `1eb2a61`; and its acceptance checkpoint is `da3e639`.
ADR 0013 was proposed and backed up at `1787d1b` before implementation.
The accepted ADR 0014 implementation checkpoint is `7663420`; final
sequence-state invariant hardening is in pushed checkpoint `ba5f513`. ADR 0015
was proposed at `488a68f`; its reviewed implementation checkpoint is
`8e0f85d`, with acceptance at `a5911d9`. ADR 0018's implementation checkpoint
is `3604345` and its acceptance checkpoint is `f1e52bd`. ADR 0019 was proposed
at `fedba98`; its architecture-neutral admission checkpoint is `cddda58`, its
wire-layout hardening checkpoint is `9977014`, and its accepted gate checkpoint
is `7d3a872`. ADR 0020 was proposed at `e7af116`; its reviewed, accepted
composition implementation checkpoint is `8da910a`. ADR 0021 was proposed at
`9835dbb`; its reviewed, accepted host-proof implementation checkpoint is
`a2c72b2`. ADR 0022 was proposed at `e85a7c0`; its reviewed, accepted native
implementation checkpoint is `fcb3578`. ADR 0023 was proposed at `6e6ac14`;
its implementation checkpoint is `83caf3f` and its acceptance checkpoint is
`e9e1f7c`. ADR 0024 was proposed at `b0f7032`; its reviewed implementation
checkpoint is `b6546cc`. ADR 0025's reviewed, accepted host-scene checkpoint is
`2857d8c`. ADR 0026's reviewed supervised-renderer implementation checkpoint is
`562d5a4`. ADR 0027 was proposed at `9d14a7a`; its host-service implementation
checkpoint is `2076eba`. ADR 0028's accepted native transport implementation
checkpoint is `228a168`. ADR 0029's native composition implementation
checkpoint is `2f453e7`, with acceptance-evidence hardening at `e605f54`.
Generated images, logs, screenshots, and debug artifacts remain under the
ignored `build/` directory.

After a power interruption on 2026-09-09, `git fsck` was clean, local and
remote `main` still matched `37fb0b7`, and the dedicated VirtualBox VM returned
to its normal bootstrap-shell image. This was recovery evidence only; the
memory ownership evidence was subsequently generated from the hardened images.

## Fixed first-circle proof accepted

ADR 0011 bounds the first isolation experiment. The feature-only x86-64 image
uses a private CR3 and one fixed ring-3 window across
three generations: exact denial of a present supervisor-only read, exact denial
of a present NX-stack fetch after a clean restart, and a terminal report from a
second clean restart. Each generation must restore the original CR3, clear its
private page tables and code, data, stack, and privilege-stack residue with
volatile writes, and verify zero before replacement.

Host checks, the strict `make circle-smoke` event sequence, normal-image
exclusion, existing QEMU regressions, and the VirtualBox run all succeeded on
2026-09-09. This is verified capability only for the fixed test image; it does
not generalize the policy or lifecycle machinery named below.

## Bounded bootstrap memory ownership accepted

ADR 0012 accepts the BSP-only ownership slice. The strict QEMU proof reported
24,162 free frames at 96 MiB and 81,506 at 320 MiB, an exact 57,344-frame delta
for the 224 MiB RAM difference. Both runs selected data frames `0x17d000` and
`0x17e000` and roots `0x17f000` and `0x183000`. The dedicated 512 MiB VirtualBox
run reproduced the lifecycle with eight normalized regions, 130,675 free and
917,901 unavailable frames, the same extents and markers, and no error or fatal
record.

Independent review drove fixes for allocator-instance provenance, unsafe
physical-clear authority, mapped-lease transfer, fail-closed framebuffer
parsing and reservation, strict transcript prerequisites, and visible packed-
pixel failure diagnostics. `make check`, all existing QEMU smoke suites,
normal-image exclusion, the two-size memory proof, and VirtualBox all passed.
The ignored `build/virtualbox-memory.log` preserves the local VM trace.

The result is ownership bookkeeping, not general isolation: it has no heap or
general process model, is BSP-only, trusts truthful firmware after structural
checks, and retains Threshold's writable/executable identity map for the first
four GiB. Direct BIOS E820 fault injection and repeated singleton publication
are not host-executed; code review covers those transitions, QEMU and VirtualBox
execute the production initializer, and kernel tests plus emulator gates cover
its fail-closed handoff boundary.

## Not implemented

The kernel does not yet have a heap or general-purpose allocator, production
scheduler, general user address spaces, process isolation, general IPC,
filesystem, package manager, or secure user shell. It has no general task/circle
runtime or production vessel supervisor. It also has no ACPI/APIC topology,
SMP interrupt routing, calibrated or wall-clock time, deadlines, general
preemption policy,
or user-space device/input model. Outside fixed test features, Rust code
executes with kernel privilege and exceptions are fatal rather than recoverable.
NMI remains masked, and the fixed exception and scheduler privilege stacks have
no guard pages. The legacy interrupt backend, provisional keyboard layout, and
bootstrap interpreter are bring-up aids, not production platform support, a
full input stack, or Canticle. The boot image is integrity-checked but not
cryptographically verified.

The accepted host session and router models are not production kernel paths.
ADR 0022 proves one removable versioned nonempty reply and exact
blocked-consumer wake, but the normal image still has no production
semantic-session transport, general wait/wake facility, deployed authenticated
semantic router, or real runtime owner enforcing orderly close before
destruction. ADR 0023 accepts only the bounded host-side routing proof.

The normal image also has no user-space compositor, window server, display
manager, deployed canonical semantic scene, accessibility service, or focus
and input policy. ADR 0024 accepts only a removable presentation feature
beneath those facilities.

ADR 0025 accepts the separate host proof for the next boundary: one exact
eight-node scene revision derives a 25-command standard-depth or 21-command
flat accessible render plan, an eight-node accessibility tree, and an
eight-node/two-action machine projection together. Its complete 96-state
matrix, atomic replacement, protected-label redaction, and exact focus/action
parity are implemented and exhaustively tested. ADR 0027 now owns this exact
profile inside a bounded host service, but neither crate is deployed in the
normal image.

ADR 0026 accepts the bounded native composition without moving that scene
authority into ring 0 or the pixel worker. Its build-time conformance step emits
a stripped redacted capsule; two supervised renderer incarnations write only
fresh kernel-owned full-frame stages; and only the successfully exited
replacement publishes. This remains a removable feature proof, not a
normal-image compositor, window server, or display manager. ADR 0029 separately
composes the live transactional surface with one successful native renderer;
ADR 0026 remains the independent bounded restart proof.

ADR 0027 accepts one host-only transactional surface service around the fixed
scene. It preserves one committed public revision while one complete successor
is queued, leased, ready, or presented; applies strict one-slot backpressure;
permits one fresh renderer retry; and commits semantics only after a trusted
presentation receipt. Its action/focus facade is revision-bound and
effect-free. ADR 0029 now retains a live instance of that transaction while
ADR 0028's fixed body drives one exact native renderer, then consumes the
private host controls only after framebuffer and independent pixel evidence.
This is still a host-run conformance bridge rather than a deployed service or
cross-process acknowledgement protocol.

ADR 0028 accepts only a removable one-source/one-consumer transport feature.
Its fixed body, endpoint, tasks, Rite operations, and grants are absent from the
normal image. ADR 0029 composes that transport with one supervised ring-3
renderer, a private stage, the framebuffer presenter, and the retained ADR
0027 transaction for one successful attempt. It does not turn the feature
harness into a native scene, window, or compositor service, nor does it define
the cross-process presentation protocol needed to deploy that composition.

The normal image therefore still has no deployed native semantic/window/
compositor service, cross-process presentation acknowledgement, scanout or
page-flip completion, GPU path, general desktop, or stable graphics ABI.
ADR 0032 now starts a separate opt-in native window-service path. Its public
window model does not yet provide application admission, cross-process client
authority, or normal-image deployment.

These omissions matter when describing the first shell: an interactive loop in
the current kernel can be a bring-up instrument, but it cannot be presented as
a secure user shell or a vessel.

## Immediate direction

The product-facing destination is a usable user-space desktop and shell with
simple commands, with the same authoritative semantics available to people and
machines. The current bootstrap interface and removable desktop feature proofs
now reach one complete native render, framebuffer verification, and paired host
semantic publication. ADR 0032 now starts a distinct native workspace boundary
with canonical window policy and software composition in one persistent
unprivileged vessel.
This preserves the fixed accepted proofs while creating room for real window
lifecycle and declarative customization.

ADR 0030 starts with explicit semantic unavailability during a presentation
transaction and a retained redacted successor for one fresh presenter to
redraw. Its host coordinator must survive. ADR 0032 adds a separate private
writable vessel and retained service lifetime with task-bound frame
presentation. Its first owner-loss policy is withdrawal
and shell fallback. Recovering windows after canonical-owner loss and deploying
independent application/client supervision still require separate evidence.

A working dependency sketch, not yet an accepted plan:

1. establish attributable faults and deterministic test entry points
   (**implemented for x86-64 CPU exceptions**);
2. prove one fixed private-CR3 ring-3 boundary and clean replacement
   (**implemented and accepted in ADR 0011**);
3. establish bounded bootstrap physical and virtual memory ownership
   (**implemented and accepted in ADR 0012; general ownership remains open**);
4. add interrupts, time, and an input transport
   (**implemented and accepted in ADR 0013; general platform support remains open**);
5. define task, circle, IPC, and sigil primitives
   (**fixed allocator-backed ring-3 proof implemented and accepted in ADR 0014;
   general facilities remain open**);
6. prove bounded preemption, private saved contexts, and one task-local fault
   with peer continuation
   (**fixed two-task PIT proof implemented and accepted in ADR 0015; production
   scheduling and general recovery remain open**);
7. prove one persistent supervisor can observe failure and replace a vessel
   (**fixed one-restart proof implemented and accepted in ADR 0018; production
   service lifecycle and policy remain open**);
8. define fixed-width rites, task-bound admission, and one native x86-64 gate
   (**fixed shell-task proof implemented and accepted in ADR 0019; the gate is
   absent from normal images and no stable semantic invocation path exists**);
9. compose one fixed human source projection and one machine-direct projection
   over the same low-level rite and supervised lifecycle
   (**implemented and accepted in ADR 0020; this is not Canticle or a semantic
   command encoding**);
10. prove bounded private semantic ingress, checked session replacement,
    lost-wake-safe waiting, and one fixed native delivery/wake bridge
    (**implemented and accepted in ADRs 0021 and 0022; production transport
    remains open**);
11. prove host-asserted source binding, explicit human focus, and an independent
    machine route without claiming credential authentication
    (**implemented and accepted in ADR 0023; deployment, credential
    authentication, and focus policy remain open**);
12. prove a ring-3 task can present bounded pixels without receiving raw
    framebuffer authority
    (**implemented and accepted in ADR 0024; this is not a compositor or
    semantic desktop**);
13. define one canonical bounded desktop scene whose committed revision derives
    both an accessible human render plan and a machine-readable semantic
    projection
    (**implemented and accepted as a host-only proof in ADR 0025; one bounded
    native composition is accepted in ADR 0029, while deployment remains
    open**);
14. compose that exact scene with one supervised ring-3 renderer and
    failure-atomic full-frame publication
    (**implemented and accepted in ADR 0026; one live-transaction transport
    composition is accepted in ADR 0029, while a deployed compositor remains
    open**);
15. preserve one private successor across strict backpressure, one renderer
    replacement, and a trusted presentation-receipt boundary before publishing
    its paired human and machine semantics
    (**implemented and accepted as the host-only ADR 0027 service; ADR 0029
    binds one retained transaction to native presentation evidence, while a
    deployed acknowledgement protocol remains open**);
16. copy one explicit redacted semantic-surface body from a separately granted
    source through one kernel-owned slot to one separately granted consumer
    (**implemented and accepted in ADR 0028; the bounded render, presentation,
    acknowledgement, and publication composition is accepted in ADR 0029**);
17. retain one live surface transaction while its exact native body drives an
    isolated renderer, a complete private stage and framebuffer verification,
    then publish its paired semantics only after an independent pixel oracle
    (**implemented and accepted in ADR 0029 for one successful attempt; this is
    not a deployed service or cross-process presentation protocol**);
18. build a persistent native window owner and software compositor with
    revisioned granted commands, public demo windows, layouts, configuration,
    complete-frame presentation, and contained shell fallback
    (**implemented and verified by native smoke and the full validation matrix
    in proposed ADR 0032; independent apps, private content, external agent IPC,
    and owner-loss recovery remain open**);
19. admit real application surfaces and an authenticated semantic IPC path,
    then add text input, a user-space shell, durable declarative
    customization, service recovery, and measured GPU/multiple-output support
    without adding ambient authority or divergent human/machine policy.

The implemented harness shortens feedback loops, accepts only bounded printable
ASCII, exposes no implied security boundary, and does not implement or freeze
the proposed Canticle syntax or ABI.

ADR 0012 accepts the first bounded portion of step 3: reject incomplete firmware
maps, normalize and reserve physical extents, grant allocator- and
generation-checked frame leases, and prove two owned non-W+X mappings at one
virtual address can be withdrawn and cleared before reuse. General physical and
virtual memory management remains a later design rather than an implication of
this bootstrap proof.

## Interrupt, time, and input delivery accepted

ADR 0013 accepts the bounded slice: BSP-only returning IRQ entry over a
replaceable `legacy_pc` PIC/PIT/8042 backend, fixed typed per-source records,
nonwrapping source and timebase identity, same-timebase monotonic ticks, explicit
loss and controlled generational rearm, and a private physical-key stream below
layout interpretation. The legacy backend is used only because `boot_omen` does
not yet carry ACPI/MADT topology.

The implementation keeps physical human input and AI authority distinct. AI
clients will later use authenticated IPC and versioned invocations; they cannot
fabricate keyboard transitions through this path. COM1 remains diagnostic
output rather than a control channel. The accepted proof supplies only the
hardware mechanism and metadata boundary—not IPC, authorization, exec-hag, or
semantic AI control.

## Fixed task, circle, IPC, and sigil proof accepted

ADR 0014 accepts the bounded experiment: fixed-capacity kernel-owned
identity and authority tables bound to allocator-backed, one-shot ring-3 tasks.
Task-local sigil references authorize copied messages to one two-slot endpoint,
while the kernel stamps provenance that user state cannot replace. Sequential
synthetic human and machine fixtures run byte-identical producer code before a
receiver drains both records in order.

`make check` passed 3 ABI, 193 kernel, and 22 verifier tests plus warning-denied
target Clippy. All QEMU regressions and normal-image exclusion passed. The
control proof produced exactly 28 records and restored 65,114 free frames under
QEMU and 130,667 under VirtualBox. The normal image is restored and running at
`bootstrap_shell.ready` with 130,683 free frames and no error or fatal record.

This result is explicitly not a scheduler, stable syscall or IPC ABI, general
circle or vessel runtime, authenticated identity system, semantic rite path,
Canticle, or exec-hag. The role fixtures provide synthetic attribution, not
authentication.

## Bounded preemption and user-fault containment accepted

ADR 0015 accepts the next fixed mechanism proof. An architecture-neutral
round-robin table composes ready, running, suspended, faulted, and exited task
transitions. The x86-64 harness binds two contexts to separate one-task circles,
private roots, and allocator-owned 16 KiB privilege stacks. Exactly two real
PIT IRQ0 deliveries preempt each task once. The first task then resumes and
executes one exact `ud2`; the kernel faults only that task and resumes the peer,
which verifies private continuity and completes.

`make check` passed 3 ABI tests, 214 base kernel tests, 220 scheduler-feature
kernel tests, and 26 verifier tests plus warning-denied Clippy. The strict
25-record QEMU proof observed ticks 1 and 2 and restored 65,112 free frames. The
exact VirtualBox VM reproduced it at ticks 2 and 3 and restored 130,665. All
prior QEMU regressions and normal-image exclusion passed before the ordinary VM
image was restored and left running at `bootstrap_shell.ready` with 130,683
free frames.

This is not a production scheduler, general preemption or fault policy, stable
context/syscall ABI, authenticated human/AI control path, vessel supervisor,
SMP implementation, extended-state switch, or denial-of-service boundary.
Successful teardown is complete; setup or teardown inconsistency is terminal
and leaves any unresolved ownership quarantined until halt.

## Bounded vessel supervision and restart accepted

ADR 0018 accepts one fixed relationship between a persistent supervisor fixture
and one restartable vessel generation. A real PIT preemption lets the supervisor
block before generation 1 resumes and faults at one exact `#UD`. The supervisor
consumes that kernel-owned observation, authorizes one replacement at the saved
kernel-stack checkpoint, observes generation 2 complete, and then exits. The
proof withdraws each generation in dependency order and returns supervision,
scheduler, task, circle, mapping, context, and allocator accounting to zero.

This is not a general vessel manager, loader, package lifecycle, health check,
restart policy, authenticated operator, or stable supervision ABI.

## Fixed x86-64 rite gate accepted

ADR 0019 accepts allocation-free, fixed-width rite prefixes, a task-bound
architecture-neutral admission core, and one removable x86-64 proof gate. The
fixed shell-task fixture models a future supervised vessel but creates no
supervision relationship. Its RX code, RW+NX mailbox, RW+NX stack, fixed
`0xf3` gate, exact running task, and kernel-selected `shell_minimum` grant admit
no arbitrary pointer or client-label authority.

The 20-record QEMU lifecycle covers nine calls in order: unknown operation,
missing console right, display-right overclaim, transport overflow, invalid
UTF-8, successful 16-byte screen commit, empty-input `would_block`, invalid exit
reservation, and terminal exit. Six failures carry no reply payload. Terminal
success copies no reply and does not `iretq` into the exited task. Teardown
restored all 65,116 initial free frames and zero active/data/table accounting.

The proof is BSP-only with IF clear and NMI masked. It does not prove concurrent
mailbox mutation, general user-copy, scheduling, nonempty input delivery,
composition with ADR 0018 supervision, a user-space shell, authenticated
human/AI identity, or a production/stable syscall ABI. The native kernel and
rite transport target remains x86-64; future IA-32 machine-code and 32-bit Win32
ABI support belongs in a user-space compatibility veil that translates into
fixed-width native rites.

## Fixed supervised shell-vessel composition accepted

ADR 0020 accepts a removable x86-64 proof that composes the previously separate
scheduler, supervision, and rite mechanisms. Generation 1 projects one exact
compiled human source into `console_write`, observes empty input, survives a
real PIT preemption, then faults at one ELF-derived `ud2`. Its persistent
supervisor consumes the observation and reaches the safe kernel-stack restart
checkpoint before the kernel retires the old rite channel and destructively
withdraws the generation.

Generation 2 proves zeroed storage reuse and starts a fresh task-bound channel
at sequence one. Its fixed machine-direct request matches the first
generation’s operation, payload kind, declared right, bounds, body, and result.
After the same empty-input result, zero-status `task_exit` completes the
scheduled task, publishes and consumes the supervision result, wakes the
supervisor, copies no reply, and never restores the exited vessel.

The QEMU proof exited with debug status 33 and emitted exactly 38 schema-1
information records. It observed PIT tick 11, five rites, two private 17-byte
console commits, one restart, one terminal exit, two retired channels, complete
storage clearing, and restoration of all 65,102 initial free frames. The
verifier rejects lifecycle and terminal mutations, private disclosure, and
release-accounting changes; normal-image exclusion rejects the complete feature.

This remains a fixed, single-BSP, NMI-masked mechanism proof. It adds no
Canticle parser, shell loop, physical input, authenticated semantic identity,
general loader or vessel runtime, production policy, SMP support, stable ABI,
or normal-image gate. The kernel remains x86-64-native; IA-32 machine code and
the 32-bit Win32 ABI remain user-space compatibility-veil responsibilities.

## Bounded private shell-session ingress accepted

ADR 0021 accepts an allocation-free, `no_std` host proof above the kernel
boundary. One session incarnation binds exact nonzero session, consumer, and
route freshness to a four-entry FIFO of bounded semantic editing actions and at
most one wait. Within a host-reserved exclusive lineage, checked close and
replacement prevent queued typeahead, prepared submissions, or stale wait
identities from crossing an owner incarnation. The two-step prepare/commit
protocol closes the check-then-sleep lost-wake window,
and revision headroom preserves orderly close plus terminal acknowledgement at
counter exhaustion.

Private content owners move rather than clone, expose content-derived values
only through named local accessors, and share one logical-clear path on
consumption, close, and drop. This does not authenticate clients, authorize an
observation sink, or claim secure erasure of caller copies, compiler temporaries,
or hardware state.

At implementation checkpoint `a2c72b2`, 51 unit tests and 56 compile-fail
documentation tests passed. The complete `make check` matrix, every established
QEMU smoke target, and normal-image exclusion also passed. No VirtualBox run was
required because the accepted crate is not linked into boot or kernel code.

The privileged mechanism boundary is accepted in ADR 0022: the kernel has a
removable nonempty semantic-session reply and exact nonterminal wake proof. ADR
0023 now accepts a host-only router that consumes an external authentication
assertion, but there is still no deployed session service, credential
authenticator, production focus service, or real shell runtime consuming the
mechanism. Physical input and semantic authority remain separate. ADR 0010 and
Canticle remain Proposed.

## Bounded native session input and exact wake accepted

ADR 0022 composes one versioned semantic-session rite, fixed private endpoint,
owned pending read, endpoint-gated scheduler wake, selected-frame reply, route
closure, and destructive teardown under one x86-64 feature proof. Immediate
delivery and blocked delivery use the same fixed body and copy-before-consume
contract. Input publication wakes only the exact blocked consumer at the FIFO
tail while leaving the publisher current; closure clears typeahead and resolves
the same pending wait once as a payload-free interruption.

The native selected completion advances revision twice atomically, matching the
host model's separately observable acknowledgement and receive without
exposing an intermediate kernel state. The strict transport publishes its
two-byte validity header last. Exhaustive differential traces, every-byte
failure injection, exact full-state snapshots, the 19-record QEMU lifecycle,
normal-image exclusion, and the all-feature final-link proof passed at
checkpoint `fcb3578`.

This does not authenticate any person, machine, device, paste source,
accessibility service, or remote peer. It supplies no focus policy, public
publisher, production session host, general scheduler, secure erasure, SMP
claim, stable ABI, IA-32 kernel, Canticle evaluator, or normal-image shell
runtime.

## Bounded authenticated-source routing accepted

ADR 0023 accepts the separate `no_std`, allocation-free, unsafe-free
`anomalous-session-router` host crate. It owns exact session routes A and B,
consumes trusted host assertions for two fixed source slots, and binds each
exact principal, source generation, fixed origin, route, and per-source request
sequence. A live human binding is required before controller activation creates
fixed A focus; the only transfer is A to B. Machine admission has no route
selector and always binds the current exact B incarnation.

Every router has a private nonwrapping process-local instance brand, so equal
public router identities cannot mint cross-router authority. Two-phase
prepare/publish revalidates source, focus, route, and exact-next request state
before session mutation and returns private ownership on rejection. Transfer
closes and clears A before installing B, invalidates held human-A work, preserves
unchanged machine-B work, and retains any terminal wake until acknowledgement.
Controller revocation does not require the source token. Once focus reaches
shared B, focus revocation refuses to retire the route until control explicitly
revokes the live machine binding.

At implementation checkpoint `83caf3f`, 24 router unit tests, 46 independent
compile-fail ownership/privacy cases, all `7^4 = 2,401` declared critical
routing traces, warning-denied Clippy and rustdoc, `make check`, every
established QEMU smoke target, and normal-image exclusion passed. VirtualBox was
not required because this host-only crate changes no boot, kernel, ABI, or image
path.

Principal attribution remains local because ADR 0022's version-1 delivery
carries origin only. The accepted proof does not authenticate credentials,
decide focus policy, provide secure attention or physical-input routing, define
a wire or kernel ABI, or deploy a production session runtime.

Desktop publication likewise remains host-local. ADR 0028 now accepts one
copied, versioned native projection of ADR 0027's fixed redacted snapshot and
freshness fields, but it does not return renderer acknowledgement or actual
presentation evidence to that host transaction. Window lifecycle, input/focus
arbitration, action execution, and general composition remain separate policy
and authority decisions.

## Bounded user-space display presentation accepted

ADR 0024 accepts a separate `display_vessel_test` image. Its fixed ring-3 task
holds only `display_update | task_exit` and maps exactly one RX code page, one
RW+NX Rite mailbox, and one RW+NX stack. The framebuffer address, pitch,
channel masks, and user mapping remain unavailable to the task. Architecture
code copies each complete body into fixed kernel scratch before decoding an
exact 40-byte `rite_display_patch_v1` header and tightly packed RGB8 payload.

At checkpoint `b6546cc`, the strict QEMU run accepted 772 calls: denied
authority, malformed flags, an out-of-bounds rectangle, 768 full-width row
commits, and terminal exit. Each rejected request preserved every byte of the
canary-backed aperture. The task-root walk proved that the complete aperture
remained supervisor-only, and teardown withdrew and cleared all feature-owned
task, channel, mapping, root, scratch, and lease state before restoring exact
accounting.

The retained strict-P6 capture matched an independent oracle at all 786,432
pixels. `make display-demo` exposes the same inspected image for subjective
end-user review. The full host and non-VirtualBox QEMU matrix,
`normal-exclusion`, warning-denied Clippy, explicit all-feature freestanding
build, and strict ELF link passed. The focused suites passed 307 display-kernel
tests and 46 `xtask` tests.

This is a bounded presentation mechanism, not a compositor, window server,
semantic scene, focus or input policy, production graphics ABI, GPU path, or
normal-image user-space runtime. The task root retains the supervisor-only low
identity branch; CPL3 cannot access the framebuffer, but this is not a KPTI or
Meltdown-class speculative-isolation claim. ADR 0025 defines the separate host
semantic revision, and ADR 0026 now accepts the host-derived redacted capsule,
supervised renderer, and staged whole-frame publication bridge through this
presentation mechanism.

## Canonical semantic scene proof implemented

ADR 0025 accepts a dependency-free, `no_std`, allocation-free, unsafe-free
host crate for one exact 1024x768 scene. Each privately branded, nonwrapping
committed revision derives its accessible render plan, accessibility tree, and
machine projection together. Public action and focus references remain
correlation data; only revision- and instance-bound move-only handles resolve
or traverse, and neither path executes an effect or grants authority.

At checkpoint `2857d8c`, 18 unit tests and 129 compile-fail documentation tests
passed. Independent oracles enumerate all 96 declared construction candidates,
accept 64, reject the 32 focus-disabled states, and verify exact 25/21 paint
budgets, eight accessible nodes, eight machine nodes, two actions, contrast,
flat-mode differences, protected-label noninterference, lifecycle, and
stale/foreign handle rejection. Separate malformed and invalid replacement
tests prove failure-atomic preparation. The complete host check,
warning-denied Clippy, every established QEMU regression, and the unchanged
`normal-exclusion` regression passed; the kernel dependency tree separately
confirms that the scene crate is absent from the normal image.

This host scene proof itself has no renderer, pixels, hit testing, input route,
action execution, authentication, secure attention, persistence, stable ABI,
or normal-image linkage. ADR 0026 accepts its use in a bounded supervised
user-space renderer composition, and ADR 0027 accepts its use in a bounded host
publication transaction. ADR 0028 accepts one fixed copied native projection;
none of these generalizes the scene into a window system or deployed desktop
service.

## Supervised semantic renderer composition accepted

ADR 0026 accepts a private build-time conformance bridge from the exact ADR
0025 scene to a separately linked freestanding renderer. Two protected-source
builds produced byte-identical complete target ELFs. The inspected sole RX
load segment is 6,469 bytes across two pages, and inspection rejects every
dynamic or relocation surface. The task runs with only a bounded RX image, one
RW+NX mailbox, a four-page RW+NX stack, and the `display_update | task_exit`
grant. Neither framebuffer nor supervisor-owned stage is mapped into ring 3.

In the strict QEMU proof, generation 1 took exactly one real PIT preemption,
staged rows 0 through 383, and reached its exact linked `ud2`. The supervisor
contained the fault and fully withdrew and cleared the incomplete generation
while the live framebuffer remained unchanged. Fresh generation 2 staged all
768 rows, exited terminally without reply, and enabled exactly one complete
frame commit. The independent oracle matched all 786,432 pixels, and exact
allocator, mapping, task, circle, channel, authority, supervision, scheduler,
context, CR3, privilege-stack, and task-switched state was restored.

`make check` passed with 329 all-feature kernel tests, 56 `xtask` tests, 8
renderer-base tests, and 9 scene-conformance tests. The full established
non-VirtualBox QEMU matrix and normal-image exclusion passed; exclusion includes
the renderer glyph fingerprint and private source sentinels. VirtualBox was not
used. This remains a removable fixed-fixture proof, not a compositor, window
server, display manager, composition with ADR 0028's native transport, general
scene protocol, or normal-image desktop runtime.

## Transactional semantic-surface service accepted

ADR 0027 accepts the host-only, `no_std`, allocation-free, unsafe-free
`anomalous-desktop-service` crate. It owns one exact ADR 0025 scene incarnation
and keeps the complete committed render, accessibility, and machine projections
public together. At most one complete successor exists, and it remains private
while queued, leased, renderer-ready, or covered by a trusted presentation
receipt. A second producer receives its complete candidate owner back rather
than overwriting, coalescing, or dropping the pending revision.

The renderer sees only a fixed, immutable, redacted snapshot of at most 1,024
bytes. It receives no scene control, projection owner, local action/focus
handle, protected source, or presentation authority. One surface-control-
authorized replacement invalidates the first renderer generation and reoffers
the same snapshot as attempt two. Old endpoints, leases, prepared
presentations, and cross-service owners remain stale even when their public
scene, surface, and renderer identifiers match. A second replacement is
refused, and dropping a progression owner performs no protocol transition.

The separate presentation control converts one exact `prepared_presentation`
into a receipt only at the trusted host boundary. Attestation still leaves the
old semantic revision public; successful receipt consumption alone commits the
prepared scene and releases the one producer slot. The public action/focus
facade delegates to the committed scene through service-branded move-only
handles. Existing handles remain usable while a successor is private and
become stale after publication; neither action resolution nor focus traversal
executes an effect.

The fixed-profile validation accepts all 64 valid scene states, rejects all 32
focus-disabled states with ownership return, and covers every 64-by-64 valid
transition while comparing the complete public projections before each
publication boundary. Directed tests cover every declared transaction phase,
one replacement from leased and ready states, strict backpressure, retirement,
owner-drop non-transitions, stale/foreign and equal-public-ID/different-brand
denial, publication freshness, protected-source values of unequal lengths,
typed snapshot-field equality, content-free errors, and compile-fail
authority/privacy constraints. This is directed bounded evidence, not a claim
about arbitrary geometry, general scene graphs, concurrency, or exhaustive
future service states.

The crate has no kernel, `anomalous-abi`, vessel, Rite, framebuffer, renderer-
image, capsule, or normal-image dependency. ADR 0026's 416-byte fixed capsule
and native proof are unchanged. ADR 0027 defines no serialized layout or
stable ABI, and its presentation receipt does not inspect pixels or prove
scanout. ADR 0028 separately accepts a copied/versioned native projection, but
delivery alone neither completes this service's render lease nor proves
presentation. ADR 0029 accepts one bounded composition in which the exact body
drives supervised rendering and verified framebuffer presentation before the
retained private controls publish. Deployment, the cross-process presentation
protocol, and window/input/focus/action policy remain separate work.

## Native semantic-surface transport accepted

ADR 0028 accepts one removable x86-64 transport proof around a fixed 528-byte
version-1 body. The core codec is `no_std`, allocation-free, unsafe-free, and
independent of scene/service policy. Its optional adapter maps the complete
borrowed ADR 0027 view through explicit named fields; the consumer validates
the whole body against an independent hand-maintained typed oracle. No private
Rust object representation, enum discriminant cast, padding, service brand, or
move-only authority owner crosses the wire.

The source holds only `semantic_surface_submit | task_exit`; the consumer holds
only `semantic_surface_read | task_exit`. The kernel binds their exact task
incarnations and a private endpoint generation to the expected surface, scene,
base/successor revision, renderer generation, and delivery attempt. Submission
copies before parsing and commits only into an empty one-slot endpoint. Read
copies before consuming, clears stale destination bytes, and publishes its
valid reply header last.

At checkpoint `228a168`, the packaged source flat image was 2,384 bytes in one
page and the consumer flat image was 5,094 bytes in two pages. Their ELF sizes
were 1,107,792 and 1,185,928 bytes. The dedicated QEMU proof completed 11 Rite
calls, 9 body copies totaling 2,176 bytes, one committed submit and delivery,
one backpressure result, two authority denials, two malformed results, two
terminal exits, and two restored kernel continuations. The source retired
before the consumer received the independent endpoint-owned copy, and teardown
restored every tracked channel, task, scheduler, endpoint, mapping, address-
space, scratch, and allocator count exactly.

Every source and destination body-copy offset was fault-injected across all 528
bytes with no premature state mutation or consumption. Two runtime-derived
protected inputs of different bytes and lengths produced identical snapshots
and source payloads. Derived-probe scans covered Cargo target trees, generated
files, ELFs, flat images, wrappers, the feature kernel, the normal image, and
the transcript; none contained the protected values. The accepted validation
matrix included `make check`, warning-denied host and x86-64 Clippy, all-feature
precedence/link and artifact checks, every configured QEMU regression, the
dedicated transport smoke, and final normal-image exclusion. VirtualBox was
not used or required.

This proof ends at one-time copied delivery. It does not render the body,
complete an ADR 0027 lease, stage or present pixels, prove scanout, acknowledge
the transaction, or publish semantic revision N+1 by itself. ADR 0029 accepts
one separately bounded composition of those later steps.

## Native semantic render-and-present composition accepted

ADR 0029 accepts one removable x86-64 conformance bridge from a live retained
ADR 0027 transaction through ADR 0028 transport, native rendering, staged
framebuffer presentation, independent host verification, and exact paired
semantic publication. The retained service exposes revision 1 throughout the
native run. Its exact ordinal-61 successor becomes the 528-byte source body;
the kernel-owned endpoint outlives the retired source and delivers that body
once to a separately linked ring-3 renderer with only
`semantic_surface_read | display_update | task_exit` authority.

The renderer copies the delivery into private owned storage, validates the
complete typed profile, and submits exactly 768 ascending 1024-pixel RGB8 rows.
The kernel holds them in one zeroed, supervisor-only 576-frame stage. The live
framebuffer stays at the declared mutation-oracle baseline until the renderer
has completed, the completion-only supervision observation has been consumed,
and all fallible source, renderer, supervisor, scheduler, channel, endpoint,
mapping, address-space, scratch, and private task state has been withdrawn.
One complete-frame copy follows preflight; kernel readback compares the visible
aperture with the exact stage and the padding with the baseline before the stage
is cleared and released.

At implementation checkpoint `2f453e7`, the strict lifecycle recorded 777 Rite
calls, 775 body copies totaling 2,392,160 bytes, 768 row commits, and one final
framebuffer commit. It restored all 65,095 initial free frames. The retained
private host coordinator then verified the closed-world transcript and compared
all 786,432 captured RGB pixels with an independent semantic oracle before it
completed the render lease, attested presentation, published, and required
revision 2 with publication sequence 2.

The inspected source flat image is 2,384 bytes in one page with SHA-256
`8bab87d92fa74a43c47321c13743bb371c176169b83cf32bcb9f089a2723f645`.
Its ELF is 1,107,792 bytes with SHA-256
`a2bcd57a22801fe821ddb089d0ecfefa2efd137facf28756878672aa1ef0c9a9`;
its 3,848-byte wrapper hash is
`3ac25fc3177f11254ab037584a135be57a139fa6734842042eb6d35a09063bad`.
The renderer flat image is 10,352 bytes in three pages with SHA-256
`666c638bd52d76b4883ae0b4bb2a4892fb02fcba0e572440db49ab950e4edc74`.
Its ELF is 1,239,056 bytes with SHA-256
`fcc9decbaddb8122063ac88a52c7dfd1da26b27b287fd4b926b1d4b491bb33ac`;
its 11,744-byte wrapper hash is
`74693a532c34d19141e6ebf0d0d240b04d98a5c5117495504ae534f6e25a5573`.

The proof covers one successful attempt only. Early exit and partial completion
are rejected by the implementation's structural and component evidence, not by
a dedicated negative QEMU run. The initial framebuffer baseline is a complete
mutation oracle, not a proven visual rendering of semantic revision 1. The
framebuffer copy and host publication remain separated by a crash gap.
Nothing here proves scanout completion, a page flip, vsync, GPU/DMA fences, a
general desktop, a deployed native service, a stable ABI, or cross-process
presentation authority. VirtualBox was not used for that accepted proof. The
new ADR 0032 workspace increment is recorded separately above; it does not
retroactively broaden these ADR 0029 claims.

## Interactive proof implemented

The first vertical slice now:

- accepts bounded PS/2 input through a host-tested decoder;
- separates a literal line classifier from built-in dispatch;
- invokes only `help`, `version`, `about`, `clear`, and `echo`;
- renders human output while emitting privacy-limited structured outcomes; and
- runs through an automated injected-keyboard QEMU test.

ADR 0022 now accepts the foundational native bridge: a separate versioned
semantic-session rite, copy-before-consume nonempty reply, owned pending read,
and exact task-local wake. Its publisher remains a kernel-authored fixture, so
the proof does not claim authentication or focus policy. Canticle syntax, a
parser, formatter, canonical AST, and effect-free round-trip vectors remain
separate proposed language work; the real shell still waits for an isolated
user-space runtime and production session host around the bounded router proof.

## Evidence pointers

- [`README.md`](../../README.md) — current supported build and boot workflow
- [`docs/roadmap.md`](../../docs/roadmap.md) — capability gates
- [`docs/architecture/boot.md`](../../docs/architecture/boot.md) — implemented Threshold path
- [`docs/architecture/kernel.md`](../../docs/architecture/kernel.md) — implemented fault, fixed isolation, memory, and interrupt/input paths
- [`docs/adr/0011-prove-the-first-circle-with-a-fixed-x86-64-prototype.md`](../../docs/adr/0011-prove-the-first-circle-with-a-fixed-x86-64-prototype.md) — accepted fixed isolation proof
- [`notebook/Development/2026-09-09 First Circle Log.md`](../Development/2026-09-09%20First%20Circle%20Log.md) — implementation and evidence trail
- [`docs/adr/0012-prove-bounded-bootstrap-memory-ownership.md`](../../docs/adr/0012-prove-bounded-bootstrap-memory-ownership.md) — accepted bounded frame and mapping ownership proof
- [`notebook/Development/2026-09-09 Memory Ownership Log.md`](../Development/2026-09-09%20Memory%20Ownership%20Log.md) — recovery, implementation review, and acceptance evidence
- [`docs/adr/0013-prove-bounded-interrupt-time-and-input-delivery.md`](../../docs/adr/0013-prove-bounded-interrupt-time-and-input-delivery.md) — accepted returning IRQ, monotonic timebase, and private physical-input proof
- [`notebook/Development/2026-09-09 Interrupt Time and Input Log.md`](../Development/2026-09-09%20Interrupt%20Time%20and%20Input%20Log.md) — implementation, human/AI authority split, and acceptance evidence
- [`docs/adr/0014-prove-fixed-attributable-sigil-gated-ipc.md`](../../docs/adr/0014-prove-fixed-attributable-sigil-gated-ipc.md) — accepted fixed one-task-circle, local-sigil, and copied-IPC proof
- [`notebook/Development/2026-09-09 Task Circle IPC and Sigil Log.md`](../Development/2026-09-09%20Task%20Circle%20IPC%20and%20Sigil%20Log.md) — implementation, review, and acceptance evidence for ADR 0014
- [`docs/adr/0015-prove-bounded-preemption-and-user-fault-containment.md`](../../docs/adr/0015-prove-bounded-preemption-and-user-fault-containment.md) — accepted fixed preemption, saved-context, and exact user-fault proof
- [`notebook/Development/2026-09-09 Bounded Scheduling and Fault Containment Log.md`](../Development/2026-09-09%20Bounded%20Scheduling%20and%20Fault%20Containment%20Log.md) — implementation, review, emulator evidence, and claim boundary for ADR 0015
- [`docs/adr/0018-prove-bounded-vessel-supervision-and-restart.md`](../../docs/adr/0018-prove-bounded-vessel-supervision-and-restart.md) — accepted fixed supervision and one-restart proof
- [`notebook/Development/2026-09-09 Bounded Vessel Supervision and Restart Log.md`](../Development/2026-09-09%20Bounded%20Vessel%20Supervision%20and%20Restart%20Log.md) — supervision implementation, evidence, and claim boundary
- [`docs/adr/0019-define-first-supervised-vessel-rite-abi.md`](../../docs/adr/0019-define-first-supervised-vessel-rite-abi.md) — accepted fixed-width rite boundary and feature-only x86-64 proof
- [`notebook/Development/2026-09-13 Supervised Vessel Rite ABI Log.md`](../Development/2026-09-13%20Supervised%20Vessel%20Rite%20ABI%20Log.md) — rite ABI, admission, gate evidence, and claim boundary
- [`docs/adr/0020-compose-a-fixed-supervised-shell-vessel.md`](../../docs/adr/0020-compose-a-fixed-supervised-shell-vessel.md) — accepted fixed scheduler, supervision, and rite composition proof
- [`notebook/Development/2026-09-13 Supervised Shell Vessel Composition Log.md`](../Development/2026-09-13%20Supervised%20Shell%20Vessel%20Composition%20Log.md) — implementation, QEMU evidence, review, and claim boundary
- [`docs/adr/0021-prove-bounded-private-shell-session-ingress.md`](../../docs/adr/0021-prove-bounded-private-shell-session-ingress.md) — accepted host-only private semantic input and lost-wake-safe waiting proof
- [`notebook/Development/2026-09-13 Private Shell Session Ingress Log.md`](../Development/2026-09-13%20Private%20Shell%20Session%20Ingress%20Log.md) — implementation, host evidence, review, and deferred kernel boundary
- [`docs/adr/0022-prove-bounded-native-session-input-and-exact-wake.md`](../../docs/adr/0022-prove-bounded-native-session-input-and-exact-wake.md) — accepted versioned semantic-session reply and exact nonterminal wake proof
- [`notebook/Development/2026-09-13 Native Session Input and Exact Wake Log.md`](../Development/2026-09-13%20Native%20Session%20Input%20and%20Exact%20Wake%20Log.md) — implementation, QEMU evidence, independent review, security boundary, and claim limits
- [`docs/adr/0023-prove-bounded-authenticated-source-session-routing.md`](../../docs/adr/0023-prove-bounded-authenticated-source-session-routing.md) — accepted bounded authenticated-source and focus-routing host proof
- [`notebook/Development/2026-09-19 Authenticated Source Session Routing Log.md`](../Development/2026-09-19%20Authenticated%20Source%20Session%20Routing%20Log.md) — implementation, security review, host evidence, regression evidence, and claim limits
- [`docs/adr/0024-prove-bounded-user-space-display-presentation.md`](../../docs/adr/0024-prove-bounded-user-space-display-presentation.md) — accepted bounded ring-3 display-presentation proof
- [`notebook/Development/2026-09-19 User-Space Display Presentation Log.md`](../Development/2026-09-19%20User-Space%20Display%20Presentation%20Log.md) — implementation, visual artifact, QEMU evidence, security review, and claim limits
- [`docs/adr/0025-prove-bounded-canonical-semantic-desktop-scene.md`](../../docs/adr/0025-prove-bounded-canonical-semantic-desktop-scene.md) — accepted host-only canonical semantic-scene proof
- [`notebook/Development/2026-09-19 Canonical Semantic Desktop Scene Log.md`](../Development/2026-09-19%20Canonical%20Semantic%20Desktop%20Scene%20Log.md) — exhaustive state, projection-parity, privacy, review, and acceptance evidence
- [`docs/adr/0026-compose-a-fixed-supervised-semantic-renderer.md`](../../docs/adr/0026-compose-a-fixed-supervised-semantic-renderer.md) — accepted fixed supervised semantic-renderer composition proof
- [`notebook/Development/2026-09-19 Supervised Semantic Renderer Composition Log.md`](../Development/2026-09-19%20Supervised%20Semantic%20Renderer%20Composition%20Log.md) — deterministic build, native lifecycle, pixel oracle, restoration, privacy, and acceptance evidence
- [`docs/adr/0027-prove-bounded-transactional-semantic-surface-publication.md`](../../docs/adr/0027-prove-bounded-transactional-semantic-surface-publication.md) — accepted host-only transactional semantic-surface publication proof
- [`notebook/Development/2026-09-19 Transactional Semantic Surface Publication Log.md`](../Development/2026-09-19%20Transactional%20Semantic%20Surface%20Publication%20Log.md) — proposal, host implementation, ownership/privacy review, and acceptance evidence
- [`docs/adr/0028-prove-bounded-native-semantic-surface-transport.md`](../../docs/adr/0028-prove-bounded-native-semantic-surface-transport.md) — accepted fixed copied native semantic-surface transport proof
- [`notebook/Development/2026-09-19 Native Semantic Surface Transport Log.md`](../Development/2026-09-19%20Native%20Semantic%20Surface%20Transport%20Log.md) — explicit wire, copied endpoint, QEMU lifecycle, privacy, fault-injection, and acceptance evidence
- [`docs/adr/0029-prove-bounded-native-semantic-render-and-present-composition.md`](../../docs/adr/0029-prove-bounded-native-semantic-render-and-present-composition.md) — accepted bounded native semantic render-and-present composition
- [`notebook/Development/2026-09-20 Native Semantic Presentation Composition Log.md`](../Development/2026-09-20%20Native%20Semantic%20Presentation%20Composition%20Log.md) — retained transaction, native renderer, private stage, pixel oracle, publication, and acceptance evidence
- [`docs/threat-model.md`](../../docs/threat-model.md) — current security gaps and required invariants
