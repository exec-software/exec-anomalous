# Executable Replacement Model Log — 2026-10-19

## Scope

Continue from accepted ADR 0097 at `81b2cde923befc841743dc7e5a60dc493d217bcd`
on `codex/runtime-executable-model`. The qualified live loader returns eleven
Linux calls and stops at sequence 12, requesting a fixed private RX mapping
inside the retained libc range. That live boundary remains unchanged here.

[ADR 0098](../../docs/adr/0098-model-source-backed-executable-replacement.md)
defines the next bounded checkpoint: inert userspace/native request validation
and a host-only permission transaction model using the original mapping owners.
It does not wire a new gate operation or execute the RX mapping.

## Resource and evidence setup

Evidence directory:
an external validation directory (not distributed).
The previous mapping-resume evidence remains retained separately. This turn has
fresh boot ID `46c78ddd-d1ca-4e5d-beae-dd7cf9008e4e` and `oom_kill 0` baseline.
Do not combine counters with the previous turn's host boots.

HAG acknowledged Anomalous's claim of the shared heavy-work window. Builds,
analyzers and virtual machines remain serial with one compiler/test thread,
MemoryHigh 4 GiB, MemoryMax 6 GiB and MemorySwapMax 512 MiB, using the unchanged
guard SHA256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
Independent source review may run concurrently; it runs no heavy jobs.

## Implementation and review

Added an inert twelve-word executable-replacement request and exact userspace
recognition of the observed sequence-12 Linux call. Twelve new protocol tests
check containment, source correspondence, padding, full-width correlation,
descriptor/snapshot identity and unchanged file-service state.

The test-only kernel model borrows original mapping owners and compares complete
simulated trees. Ten tests cover the actual 408-page range, every attempted leaf
store at false-before-write, false-after-write and missing-readback positions,
unaltered physical addresses and neighbours, exact private ancestry, correlation
and epoch mismatch, both invalidation phases, and terminal cleanup after partial
progress. Physical model RAM, allocator accounting and original RO/NX plans are
unchanged by replacement operations. Independent source review found and fixed
cross-owner physical overlap, source-origin/grant consistency, and the need to
state prior graft hardware admission explicitly before permitting Accessed bits.

These tests do not establish actual source-owner retention, common allocator
lineage, hardware permissions, TLB invalidation, sealed scheduler authority or
saved-frame preservation. Epoch equality applies within one model; freshness
across models remains an adapter obligation.

## Qualification in progress

The first guarded attempt `focused-v1` stopped at a protocol test-fixture lifetime
compiler error (E0515). The fixture now borrows caller-owned namespace entries;
no leaked allocation or fabricated static service lifetime is used. The failed
logs and original source snapshot remain intact and do not count as qualification.

The repaired source was formatted and frozen before testing in `focused-v2`.
Its tests passed, then strict protocol Clippy rejected three redundant bit-mask
expressions in boundary tests. Those expressions were simplified with the same
values. The failed lint attempt is retained; `focused-v3` is the complete retry.

`source-freeze-v3.json` binds all 572 non-Markdown source files; SHA256
`f3255f214d7afffe03a2f507d533d2c992d2519f2ef1f8c96fc0339b43de1e83`.
Its recorded HEAD is the prior accepted documentation commit; the pending source
checkpoint must match those exact file bytes. Protocol all-feature tests passed
101/101 and explicit kernel library tests with `linux_loader_entry_test` passed
661/661, including all 22 new tests. The final `focused-v3` run passed all seven
jobs: formatting, source freeze, protocol 101/101, kernel 661/661, xtask 390/390,
protocol all-target/all-feature Clippy and freestanding kernel Clippy, both with
warnings denied. The test-only model is compiled/executed by the explicit kernel
library tests; freestanding Clippy does not lint that test-only module.

Full repository validation, image comparison and final resource audit remain
pending at this source checkpoint. No live RX qualification is claimed.
