# Linux startup and record-device application comparison

Date: 2026-10-04

## Scope and final status

[ADR 0052](../../docs/adr/0052-prove-linux-startup-and-record-device-syscalls.md)
qualifies a real Linux x86-64 startup/SYSCALL route for an independent editor
producer paired with the existing native INT f4 service. All 37 final matrix
jobs and both image-restoration jobs passed on 313 unchanged frozen source
files. The ADR remains Proposed. The preceding ELF proof's 35 matrix jobs and
two restorations remain historical evidence; this slice has its own results.

This is a feature-private proof-controller bridge over shared native mechanisms.
Moving its foreign-platform policy into a supervised userspace veil is the next
architectural gate under [ADR 0008](../../docs/adr/0008-isolate-foreign-platform-compatibility.md).
The producer is a small Rust client using the established editor contract,
not the Electron Antimatter application. Ny and HAG's accepted direction and
independent qualification gates remain unchanged. Displacer remains deferred;
no inspection or implementation was undertaken. Threshold is unchanged, and
VirtualBox was not rerun.

## Implemented boundary

The shared allocation-free, unsafe-free ELF adapter still runs in an independent
ring-3 loader. It parses one canonical sectionless static ELF64, emits a copied
96-byte inert plan and grants no authority. Kernel admission checks exact
trusted-manifest equality and the current loader identity. Loader resources
are reclaimed before application allocation. Only two tasks are live at once;
the comparison runs two loaders and eight application tasks across the native
and Linux normal/fault routes, retaining the existing 50-frame peak ownership.

The Linux producer's RX starts at 0x400000 with a 32-KiB bound. RW starts at
0x800000 with an 8-KiB bound. Initial RSP is stack top minus 256, 16-byte aligned,
inside the existing guarded 8-KiB stack. Startup carries argc=2, argv
(`anomalous-editor`, `normal` or `fault`), NULL terminators, empty envp and only
AT_PAGESZ=4096, AT_ENTRY=0x400000 and AT_NULL. There is no invented AT_PHDR,
random seed, credentials, CPU vector or vDSO. RDX is zero; stock libc startup
is not admitted.

An owned entry session captures EFER.SCE, STAR, LSTAR and FMASK before mutation,
selects trusted supervisor storage before any push or call, normalizes the
176-byte retained frame and returns through checked IRETQ. SCE is enabled only
for the Linux producer. The native service retains INT f4 with SCE disabled.
Complete MSR/CPU restoration, cleared scratch state and terminal ownership
withdrawal precede release. This remains the sole-BSP, NMI-excluded, zero-GS,
eager x87/SSE2 profile; it adds no TLS, AVX/XSAVE or SYSRET boundary.

The supported raw calls are read=0, write=1, poll=7 with timeout zero,
sched_yield=24 and exit=60. Syscall numbers use signed-low-32-bit normalization:
the actual client probes `0x100000000`, which selects read. FD/count/timeout
widths, raw negative errno and unknown-number ENOSYS are separately checked.
FD3 is a write-only surface route and FD4 a read-only input route. They are
**nonblocking Anomalous record devices, not Linux pipes**; the exact current
task identity authorizes the binding, and packet bytes remain opaque to the
kernel. No filesystem or general descriptor allocation is introduced.

Generic `application_channel::consume_with` checks the branded delivery token,
completes the checked destination copy, then clears the queue through an
infallible commit. Failed copies preserve queued bytes; successful reads release
backpressure without a second fallible acknowledgement. Poll snapshots up to
eight entries and preflights the entire result array before any write. Host and
guest probes cover readiness, duplicates, ignored negative descriptors,
unconditional ERR/HUP/NVAL bits, wrong direction, zero/oversized lengths,
undersized reads, copy failures and preserved poll sentinels. The ADR records
the exact device-specific errno and state table.

## Observed application and entry results

TCG with Nehalem/GenuineIntel and KVM with host/AuthenticAMD passed at 96 and
320 MiB. Each configuration produced the exact 25-record transcript: the
existing 21-record native baseline followed by the loader result, two Linux
scenario records and the final named-personality comparison. The loader
reported 15 calls, one denial, one bad copy, eight invalid offers and one
accepted 96-byte plan, including a decoded-valid but unauthorized manifest.
Initialized RW readback, zero-fill and complete loader reclamation passed.

| Linux route | Total calls | read / write / poll / yield / exit / unknown | Denials / bad copies / invalid |
| --- | --- | --- | --- |
| normal | 132 | 12 / 11 / 32 / 7 / 1 / 2 | 2 / 5 / 8 |
| queued UD2 fault | 12 | 1 / 3 / 0 / 0 / 0 / 2 | 4 / 1 / 2 |

Normal execution has 65 Linux calls and 67 native service calls, 14 cooperative
switches, seven complete surface records and six input records, with matching
deliveries and acknowledgements. The fault route has six calls from each
personality, no cooperative switch, one queued surface discarded, two closed
routes, one contained fault and one surviving peer exit. Timer counts are
bounded observations, not required equal across backends. Free frames returned
to 24,113 at 96 MiB and 81,457 at 320 MiB, an exact difference of 57,344.

Each comparison preserved editor hashes, pixels, resource cleanup and the last
frame after fault. The existing native baseline and Linux route each commit a
final presentation; the emitted native presentation record describes its one
baseline commit, while the complete comparison has two. Separate host-quit
capture runs verified 25 records and all 9,216 pixels of the 384×24 region at
(32,32) in a 1024×768 framebuffer. Outside-region preservation passed, and the
whole retained PPM files match byte-for-byte between TCG and KVM. These capture
runs are explicitly separate from the two debug-exit boots per backend.

The actual assembly probe checked -ENOSYS, architectural RCX/R11, unchanged RSP,
12 non-result GPR canaries and every byte of the 128-byte leaf red zone. The
normal syscall site is 0x40001c, the probe site 0x400121, and the exact admitted
UD2 fault is 0x400020. A different failure UD2 cannot satisfy the fault oracle.
The host Antimatter CodeMirror reference check also passed; it remains a
behavioral reference outside the VM, not evidence of an Electron guest port.

## Artifact and emitted-code binding

| Final artifact | SHA-256 |
| --- | --- |
| Linux proof image | `82a43eb555d5ad7557e590df3032f49dd02b9be8e8a9ac63420deac854510e9a` |
| Linux proof kernel | `78330b97c8bfc95ef37a618f3fba8ea0e80325f101a948e01b5fac61124ec115` |
| Linux client ELF | `dcf67bad21c8ed7a724b7b08dbc148996d3e9e1ebbc1d0bbb4fab47fea46abbb` |
| Loader ELF | `3cff9fabebb568488c86a40e02b7811dc09a4822fa8ccbd119ee78ca73cce420` |
| Sectionless deployment ELF | `6e68a3abf020d026140a217bdb848ad5c0f15eb29457748a5d727b50117bc3f9` |
| Restored normal image | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| Restored workspace image | `d6c38325c57d33677aca118772f3cf69036293b73bed1bc1bec1e3c275ecd546` |

The Linux client has 7,867 RX bytes, 136 file-backed RW bytes and 8,192 RW memory
bytes. Deployment length is 12,424 bytes. Its actual initialized canary at
0x800080 and the reserved poll tail at 0x801ff8 survive code generation. The
loader has 15,864 RX bytes and 128 RW bytes. Host inspection validates exact
permissions, complete admitted instruction coverage, the two SYSCALL markers,
readonly nested copies and absence from normal images. It does not establish
control-flow integrity or hostile executable admission.

Independent emitted-code inspection confirmed that the loader reads the
embedded runtime pointer/length and executes the ELF parser; optimization did
not replace it with a constant plan. The final reviewed kernel is byte-identical
to the qualified kernel. Its SYSCALL entry stores untrusted RSP in supervisor
scratch and loads the retained supervisor stack before the first push. Review
also followed SCE disable/selection and complete MSR restoration ordering.

Conservative emitted stack estimates are 6,279 bytes for Linux dispatch and
7,079 for the native gate, including the normalized frame, alignment and entry
call, against each 16-KiB retained transition stack. The known bootstrap
construction chain estimates 49,424 bytes against 65,536. The existing 16-KiB
preparation-entry headroom check passed; no stack was enlarged. Userspace uses
at most an estimated 760 bytes below initial RSP for the client, including the
red-zone probe, and 912 bytes for the loader; the client also reserves the top
256 startup bytes. These are inspected fixed-profile estimates, not formal
maxima for arbitrary fatal or concurrent execution. Retained disassembly and
manual review cover constant indirect calls, bounded local jump tables and
entry/exit stack switching.

## Validation, retries and memory

The final 37-job matrix passed in 1,453.91 seconds of summed job time. Its fresh
`make -j1 check` passed in 240.06 seconds, including 469 Linux-profile kernel
tests, 503 all-feature kernel tests, 149 xtask tests and nine shared ELF parser
tests. The matrix also includes the host reference, TCG Linux/ELF/native/Ny and
application-profile proofs, all retained boot/fault/input/display/semantic and
desktop/workspace regressions, normal-image exclusion, and KVM repetitions of
the relevant ownership and application paths. Recorded final KVM invocations
total 25, including three Linux runs. Two subsequent restoration jobs passed;
normal/workspace images match their preceding qualified bytes.

Early attempts are preserved separately. The first smoke compile exposed an
inaccessible validator helper and was fixed locally. Full-check attempts then
caught module formatting, a default-host cfg import, and a native-only unused
binding. None of those partial runs counts toward the final 37 jobs. The final
source set was frozen again and every job rerun successfully. All 313 frozen
non-documentation files remained unchanged through completion.

Builds used two jobs within a 4-GiB soft/6-GiB hard cgroup envelope. The final
matrix peaked at 2,480,099,328 bytes (2.310 GiB); minimum available host memory
was 21,713,104,896 bytes (20.222 GiB). All final scope event counters stayed zero,
and host OOM kills stayed 0→0. Restoration peaked at 29,605,888 bytes. Across
preliminary attempts, the largest scope peak was 4,296,966,144 bytes (4.002 GiB).
The native-Clippy retry recorded 916 soft-reclaim events with file cache the
majority of its usage; it recorded no hard-limit or OOM events, and observed
group swap was zero. The final run had no soft-reclaim events. Existing host
swap usage is distinct from swap attributed to these scopes.

Local retained evidence is under
an external validation directory (not distributed):
`final-receipt.json`, `matrix-final-results.json`, `restore-final-results.json`,
`source-freeze.json`, memory summaries, exact TCG/KVM artifacts/transcripts/
captures, `stack-review.txt`, `userspace-stack-review.txt`, analyzer outputs and
disassemblies, plus the checksum index. Completion checking found no QEMU or
bounded runner still active. The evidence index may grow with documentation
receipts; the final source and matrix counts above are fixed.

From the repository root, `make linux-application-inspect`,
`make linux-application-image` and `make linux-application-smoke` reproduce this
feature profile with the pinned development toolchain. Backend-specific final
runners and their exact arguments are retained with the evidence.

## Source basis and remaining gate

The [x86-64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/low-level-sys-info.tex)
defines the initial stack layout. Linux's
[ELF loader](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/binfmt_elf.c)
shows the richer environment this proof does not reproduce, and
[entry assembly](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/entry/entry_64.S)
defines RCX/R11, unchanged user RSP and signed-low-32-bit syscall normalization.
[Linux poll](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/select.c)
provides event filtering and per-entry counting. Its sequential copyback does
not establish this proof's stronger whole-array failure atomicity.
[Linux VFS I/O](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/read_write.c)
delegates to file operations; record boundaries, packet size and revocation
remain declared Anomalous device policy.

The next gate is a supervised userspace veil that owns foreign policy while
preserving the qualified mechanism and service boundaries. No result admits
arbitrary ELF/Linux binaries or claims a stable ABI, general POSIX, Linux pipes,
libc, dynamic linking, signals, threads, TLS, production security, full
Antimatter/Electron, Windows or macOS support. Frontend expansion, persistence,
Ny runtime adoption, HAG transport and Reliquary packaging retain separate gates.
