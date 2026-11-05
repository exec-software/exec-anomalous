---
title: "Linux Loader Live Path Log"
date: 2026-10-11
tags: [linux, elf, userspace, loader, copy, namespace, qualification]
status: qualified-bounded-proof
---

# Linux Loader Live Path Log

This increment observes the live pathname of the unchanged interpreter's fourth
syscall, an `openat` request. It retains three actual Linux returns and retires
without a fourth return or a file-service answer. The target remains the existing
Linux Antimatter/Electron build, with unchanged dynamically linked `true` completing
startup and exiting as the nearer gate.
[ADR 0083](../../docs/adr/0083-observe-the-loaders-live-pathname-before-file-service.md)
defines this observation boundary. The
[Linux Loader Cache Log](2026-10-08%20Linux%20Loader%20Cache%20Log.md)
preserves the preceding qualification and unknown mutable pathname.

## Resumed work

Work began on October 8 and was interrupted before any guarded build, test or
VM scope ran. Commit `65947ff8d3388918c0b2c61a5b77cc01d985b413` (`Sync`)
preserved the in-progress kernel, reader and oracle changes. Qualification
resumed on October 11 after confirming a clean checkout and no active task
units. The original preparation record remains `turn-start-interrupted.json`;
the fresh `turn-start.json` bounds today's qualification interval. The five
original baseline images still matched at resume. This does not claim memory
telemetry for the intervening idle days.

## Live copying and retirement

The existing native operation 18 now admits suspended phases `(2,1)`, `(3,2)`
and `(4,3)` expressed as raw calls/prior returns. A pure hostile cross-product
test excludes brk, already-returned, extra and overflowing phase combinations.
This phase predicate grants no authority: the current veil/frame/root/TSS and
sealed pending caller still authenticate every operation.

The userspace reader admits syscall 257 with signed low-32-bit dirfd `-100` and
low-32-bit flags `0x80000`, ignoring noncreating mode and unused arguments. It
copies checked one-byte increments and stops at the first NUL. The returned
count includes NUL. Empty and non-UTF8 contents are observations, not namespace
answers. Shape, inaccessible-memory, resource, overflow and budget errors remain
distinct in the pure reader; the native proof routes incomplete capture through
its existing failure instruction. Only a complete capture permits retirement,
which still returns no result to Linux. The private v3 policy is unchanged.

The six negative copy probes repeat under fresh sequence 4 before its byte
reads. Exact error/result words and the unchanged 24-element `i32` sentinel
(96 bytes) remain checked. Read-only-destination protection comes from the
complete destination preflight, not the separate writable sentinel alone.
Every attempt preserves all twenty-two blocked caller-frame words, the copied
request and pending sequence. The complete retained 4096-byte scratch is
volatile-cleared and read back on every returned outcome.

A terminal frame pair brackets the entire fourth capture, beginning before the
first rejected probe. Each later attempt and terminal completion checks the
same frame before consuming pending authority. The three retained pathname
journals now represent sequences 2, 3 and 4; the brk reply reports an explicit
empty journal. This reuses its former empty slot rather than adding a fourth
journal. Active copy state clears before task retirement and all archived
journals clear after restoration. Returned diagnostic snapshots deliberately
retain evidence bytes; the retained-journal wipe claim does not cover them.

The oracle requires eighteen ordered records and an independent terminal-copy
record. It checks complete equal terminal frames and correlation with the
fourth raw request, sequence/pointer binding, one unique complete owned RW/NX
stack mapping, checked range arithmetic, canonical lowercase byte encoding and
the first NUL exactly at the end. It does not compare mutable bytes with the
original source, impose ASCII, or predict a library name. The two earlier
immutable literal journals keep their separate source/policy comparisons.

For `N` terminal bytes including NUL, expected totals are four raw calls, three
returns, `65 + N` native calls, `54 + N` copy attempts, `36 + N` successes and
eighteen rejections. The physical runs below confirm these totals. The
256-byte invocation budget remains a removable experiment
bound, not a native pathname or depth limit.

## Qualification

Final sources were frozen before focused validation. Independent source reviews
cover authority, failure atomicity, fresh journals, terminal frame preservation
and the oracle's distinction between immutable source and live stack bytes.
The first focused attempt passed 612 kernel, 352 host-tool and twenty policy
tests plus policy/kernel warning checks, then host Clippy rejected a collapsible
nested condition in the new stack-ownership oracle. The condition was collapsed
without changing evaluation order. A second source freeze preserves the same
482-file inventory with only that oracle file changed. The failed attempt and
its complete guard records remain separate from final qualification; no OOM,
pressure abort or runner error occurred.

The fresh focused v2 run passed all eight jobs: 612 kernel, 352 host-tool and
twenty policy tests, policy/kernel/host warning checks, image construction and
artifact capture. The policy tests include eighteen reader/policy groups plus
the two base protocol tests. All 482 final frozen source files remain unchanged.

The native compiled graph closes all 25 sized text functions without indirect
branches/calls, recursion or dynamic stack adjustment. Its worst stack path is
1,080/8,192 bytes, leaving 7,112. Exact emitted paths verify the third negative
probe set, checked capture shape/addresses/budget/first-NUL stop, capture failure
before retirement, all three checked completion returns and no fourth return.

The fresh kernel graph closes 76 register-call sites and seven indexed tables.
Bootstrap use is 46,432/65,536 bytes, leaving 19,104 bytes and 2,720 beyond the
required separate 16 KiB reserve. This grew 1,760 bytes from the prior cache
proof despite adding only a 352-byte returned frame pair; future observation
growth needs another review. Including the 176-byte saved frame, SYSCALL,
native, timer and bounded fault-prefix paths remain 5,296, 5,184, 3,296 and
2,448 bytes of their 16 KiB stacks. Generic fatal recursion, serial-format
vtable recursion and NMI nesting remain outside this certificate. This is not
a whole-program stack bound for the foreign interpreter.

Ten shared entry/TLS paths normalize identically to the prior cache checkpoint.
Fresh emitted inspection binds the first terminal snapshot before all probes,
per-read authority/request/frame checks, equal terminal frames before scheduler
commit, journal reset before retirement and final retained-journal clearing.
The 4096-byte scratch remains in retained BSS. Embedded kernel blobs are bound
to their exact read-only section bytes; their containing ELF LOAD flags are RX,
which alone does not establish non-executable runtime mapping.

Fresh focused artifact SHA-256:

- Kernel ELF: `14495e2ac916e4f2c9da6844315978cbdc9f8f542d6d41f16c3e17b2f5ba3e58`.
- Disk image: `07e0eeed9ef0f9d71d7e0534e7a58489b5df5655d4334ab5571337ee1905b9b8`.
- Native veil ELF: `6f8249ef6abe11a2f270bc27c0dc02d5ffb07d435bef2774866adb91b2415138`.
- Unchanged policy: `dc0039b851f0b9578f5d3a57036f92cdad8a603217396b5066f901e466e5054e`.
- Unchanged plan: `b971aaf469ff272c81d3fba60e375d8f8d0059d7c46685b7b294ee0e58d5aee1`.
- Unchanged source snapshot: `b79d9ccc61e497bd773d199f430d4522278817b82e2d45de60d66219a345af90`.

TCG and KVM at both 96 and 320 MiB passed using the exact focused kernel/image
bytes. Every eighteen-record transcript observes the same actual fourth name:
`/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/libc.so.6`. This equality is a
measurement of these four runs, not a requirement that different CPU exposures
must choose the same library search path. All 55 copied bytes, including NUL,
come from the live owned stack at `0x7fff0000eec0`; both complete terminal
frames are equal and correlate to the fourth request at `0x60029743`.

Each run reports four raw calls, three Linux returns, 120 native calls,
109 copy attempts, 91 successful bytes and eighteen rejected probes. The fourth
invocation accounts for 61 attempts, 55 successes and six rejections. Scratch
and journals clear, all three tasks retire, and source/mapping/TLS/CPU owners
are restored. No fourth result is returned to Linux.

The full matrix passed all 29 required/affected targets, including the native
editor/workspace, shared-veil, ELF relaunch and Linux TLS regressions. Three
separate malformed-source boots rejected missing input, wrong length and a
one-byte corruption with `linux_source_missing`, `linux_source_size` and
`linux_source_hash`. These tests used the final pinned image/source/plan and
actual TCG at 96 MiB; the outer provisioning runner's label is host.

VirtualBox 7.2.6 booted the exact final image at 320 MiB, then refused
`application_cpu_amd_error_pointers` before userspace. Its isolated VM was
powered off. The failed harness exit is retained and classified as an
unsupported guest capability, not a semantic pass.

An independent byte-for-byte comparison of all five complete 16 MiB baseline
images passed: normal, dispatcher workspace, owned workspace, CPU workspace and
native editor workspace. Boot records, checksums, loaded kernels, padding and
disk tails remain identical. The retained generic ELF/bin/debug files are
attributed only to the final CPU-workspace image. The comparison receipt is
`cc09569b9ee126ae930ec3d72c092d8d6b791bef2829f416e77fceb60b063f61`.

The final evidence audit passed at `2026-10-11T14:45:40.361892+00:00`, binding
44 successful jobs across six selected scopes, all 29 required/affected targets,
six artifact sets, four eighteen-record transcripts, exact source/provisioning
inputs, independent source/emitted reviews and all five image comparisons.
Both unsuccessful attempts, focused v1 and VirtualBox, remain separately
retained. The manifest SHA-256 is
`cb4ce247b50ec7edbdac559a0a0a08eea4373ce11b386a184397770450ec5596`;
`final-audit-v1/receipt.json` is
`798adf8a0def7e33e32539ddf8af0ebc01b8cbd6ef993e050331f25f382bebf0`.
The native permissions clarification is included in the manifest; the original
review's containing-LOAD wording was preserved and corrected by that separate
record, without changing source or binaries.

## Memory and evidence limits

All eight attempted scopes ran serially under the unchanged guard, with one
build worker and one test thread. Each has a stopped monitor and final sample.
Limits remained MemoryHigh 4 GiB, MemoryMax 6 GiB and MemorySwapMax 512 MiB,
with a 4 GiB host-available floor, thirty quiet startup seconds, one-second
sampling and two-second rolling PSI checks across the scope and its ancestors.
No pressure abort, guard error, scope-cgroup high/max/OOM event or host OOM-counter
increase was observed. Peak scope memory was 3,755,909,120 bytes (3.50 GiB);
minimum host available memory was 18,994,454,528 bytes (17.69 GiB). Maximum
observed host swap use was 4,382,720 bytes; it is not a scope-swap attribution.

The audit recomputed pressure decisions from retained cumulative samples and
checked exact job serialization. The scope journal exposed all eight attempted
units; successful units had already been collected from the live unit inventory.
Kernel journal records were accessible and partial; systemd-oomd returned no
accessible records. These observations do not establish complete host history
or continuous pressure visibility between samples. The interrupted October 8
preparation does not provide telemetry for the idle period through October 11.

Evidence is retained under
an external validation directory (not distributed).
Prior `linux-loader-cache` evidence remains unchanged. Old seventeen-record
logs copied into a build-only artifact are stale and do not qualify the new
image. Generated images, logs and audit files stay outside Git.

## Next service

The actual fourth pathname is now an authenticated live observation. A
namespace answer must follow an explicit guest configuration. The current v3
policy declares only the preload and cache names absent; it says nothing about
this hardware-capability variant. A future configuration may explicitly omit
that variant and observe the next request, or bind compatible held-file bytes
to it. Neither host filesystem state nor the name alone supplies that authority.
A successful open needs a pinned held file and task-bound descriptor authority.
Subsequent read, metadata, mapping/protection and close behavior must follow real requests.
The held file need not be mapped in full to supply bounded reads. Dependency
loading, TLS/libc startup, a Linux exit and the larger Electron process/display
environment remain open. This increment does not establish Antimatter launch
or rendering.

The frozen `next-loader-service-handoff.md/json` records every actual pathname,
complete terminal frames, counters and bundle hashes; `next-file-service-context.md/json`
records the existing source/copy/descriptor prerequisites and bounded resource
limits. Neither substitutes for provisioning an actual libc file.

Implementation was resumed from the user's preserved
`65947ff8d3388918c0b2c61a5b77cc01d985b413` (`Sync`). The strengthened reader
tests, whole-stack ownership oracle and final source checkpoint are committed
and pushed as `389bc800c65dcb70223cfbdf72b66d1b5771ebfb`. All 482 source
hashes at that checkpoint match the audited freeze; this qualification record
and status updates follow in a documentation-only commit.
