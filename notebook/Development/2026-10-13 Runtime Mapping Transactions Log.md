---
title: "Runtime Mapping Transactions Log"
date: 2026-10-13
tags: [kernel, memory, ownership, linux, elf]
status: qualified-host-mechanism
---

# Runtime Mapping Transactions Log

## Scope

Starting checkpoint: `7feb644b655de890fffa70ae12035afd18754fe3`, clean and
synchronized with `origin/main`. The next observed Linux request is the libc
mapping retained under ADR 0089. This increment implements the retained kernel
transaction prerequisite under
[ADR 0090](../../docs/adr/0090-retain-runtime-mapping-transactions.md).
It does not answer that guest request yet.

The existing allocator borrow encloses guest execution, and committed launch
mapping access expressly excludes rebuilding or modifying payloads. Review
selected a separate isolated owner with real leases and explicit publication
and withdrawal states. Connecting it to the live caller also requires combined
root, copy, circle and cleanup invariants; changing only a capacity would omit
those obligations.

## Evidence and resource policy

Evidence is retained at
an external validation directory (not distributed).
The runner is copied unchanged from the preceding qualification, SHA-256
`19e3b5388e9f14624a8faaf726a941f1f816b1fcab75f5a92531802afc0a0d0a`.
Builds and VM runs stay serial in `anomalous-runtime-mapping-*` user service
scopes, with MemoryHigh 4 GiB, MemoryMax 6 GiB, swap 512 MiB, one Cargo job,
one test thread, a 4 GiB host-available floor and the existing ancestor/host PSI
thresholds and 30-second quiet admission. All attempted scopes, including
failures, must appear in the final audit.

HAG requested a quiet window during initial source review. It explicitly
released the host before any Anomalous heavy work started, then confirmed that
its unused watcher had exited and all workload services were retired. Source
review, documentation and test preparation continued during that hold.

## Qualification

`focused-v1` passed all six guarded jobs: formatting, the 490-file source freeze,
626 kernel tests (including fifteen new independent transaction tests), default
and loader-profile freestanding Clippy, and host test Clippy with warnings denied.
The source freeze is
`7457cfa3c0424eca43595829007f05b7b0771ed9a31e1e3a469835058c1057ec`.

The full-size fixture uses the real sealed-source and frame-lease owners over
simulated RAM, with independently generated deterministic bytes matching libc's
observed length. It walks all 545 pages, verifies all 2,190,608 initialized bytes,
the 752 zero bytes, ten absent pages, exact permissions and distinct physical
pages. Source reads stop at the logical EOF. Existing mapping and source bytes
remain unchanged through preparation and cleanup. This is a libc-shaped host
fixture, not actual libc execution or a suspended-caller frame test.

Smaller fixtures cover allocator exhaustion, every allocation clear and table
write, the first/last entry of every table in both validation passes, partial
source reads and destination writes, corrupt data readback, table corruption,
population unwind, known publication rejection/retry, unknown/unwound publication,
forbidden replay/cleanup and every cleanup clear/retry point. The existing mapper
tests provide exhaustive underlying table-readback-position coverage. A short
raw request may initialize all bytes of its final page; initialized length is
bounded by the rounded extent, not the raw length. This avoids imposing false
zero padding on valid file bytes in a future Linux adapter.

Independent final source/test review found no blocking issue; its receipt binds
all three changed Rust files to that source freeze. The owner remains dormant
outside host tests.

`loader-image-v1` built successfully but its whole-ELF identity assertion failed:
the new `kernel.elf` is
`626030203b7e3bdc45972e69b8e80af86c94a76058ce4f73ec17c7a3b7e97992`,
whereas the prior reviewed ELF is
`9272c4e4c883310163fca3d841b44e6bdf3c17fe8c751426efbf932478bb7c4c`.
That failed comparison is preserved. All nineteen other artifacts, including
`kernel.bin`, the boot image, native ELF/disassembly and sources, are exact.

The separate guarded `loader-image-v2` comparison passes equality of entry,
loadable segment addresses/flags/file and memory sizes/content, and all allocated
section content and geometry, including BSS. Its normalized section comparison
reports changes in `.debug_info` and `.debug_line`. This establishes loaded-image
equivalence, not whole-file ELF equality or an exhaustive account of every
nonloaded header/offset difference. The old emitted-review receipt remains bound
to the old ELF; no receipt is rebound to the new hash. The retained boot image
is still
`3aad3e5c3b9e7cd18fa3b2c0c21ce8b2d83d0cef722583d6b44b484765582923`.

The initial `qualification-v1` attempt reached the 900-second `make check`
deadline during strict kernel lint after successful host tests and image builds.
It was terminated by the runner, not a failed assertion or OOM. Its single
attempted job remains failed in the evidence. The scope reached its 4 GiB soft
limit and reclaimed build cache; hard-limit and OOM counters remained zero.
`qualification-v2` gives `make check` 1,800 seconds while preserving every
memory/pressure/concurrency threshold and reruns the full required matrix.

`qualification-v2` passed `make check` in 581.07 seconds and the next 25
regressions. During `native-editor-workspace-smoke`, the unchanged guard observed
host-wide full memory pressure of 14.712374568477468% over its rolling two-second
window, above the 10% limit. It terminated that job at 02:01:43 UTC on October 14.
The failed attempt remains separate: no test assertion failed, but its workspace
proof is unqualified. Scope hard-limit and OOM counters stayed zero; the unit
exited with status 1 and its cgroup retired. The guard was not relaxed.

HAG requested another quiet window before that abort and then confirmed it was
using the released host. Anomalous held all further heavy work until HAG
explicitly released it after native/UI retirement and watcher shutdown.
`qualification-v3` reruns the interrupted workspace check and the three
unstarted jobs, retaining the 26 completed checks from the identical frozen
source. No already-passed check needs replay merely because an independent
later job was pressure-aborted. HAG's test outcome remains separate from this
qualification; no cause is attributed to the observed host pressure spike.

`qualification-v3` passed all four remaining jobs after the explicit quiet-window
release. The fresh workspace proof is
`build/native-editor-workspace-proof-3w9mvfkr`; the interrupted proof remains at
`build/native-editor-workspace-proof-b_ergecm`. All 29 required/affected targets
and CPU workspace restoration are complete. The five development images are
byte-identical to the turn-start baseline. No new VirtualBox run is claimed;
its previously recorded CPU capability refusal remains separate.

The final audit covers six scopes and 41 attempted jobs, including all three
failed attempts. Guarded job intervals do not overlap. Peak scoped memory was
4,297,060,352 bytes (about 4.002 GiB, including cache); the initial broad check
reclaimed cache at MemoryHigh, with no hard-limit event. Minimum sampled host
available memory was 13,672,210,432 bytes (about 12.73 GiB). All scope OOM and
hard-limit counters remained zero, and the host OOM-kill counter was unchanged.
There was one guarded host-pressure abort, as recorded above. The resumed scope
peaked at 2,094,575,616 bytes with no pressure abort or OOM event.

`final-audit-v1.json` passed, SHA-256
`f9ef4fa0509e29131498182e02e002b4d435673934cd71486aa863a53b7d4cda`.
It binds the 490-file source freeze, exact required/affected successes, focused
tests, source review, loaded-image comparison and
`memory-audit-final-v1.json` (SHA-256
`51d29f70a3ac667f42cb4e9400a4562a94ac27c7cfe30fa36531f413458cffea`).
Independent review also checked the audit's handling of the deadline and exact
pressure abort; neither failed scope is relabeled as passed. Implementation
checkpoint `b28c27e` and the interrupted-run/handoff checkpoint `122fcea` were
committed and pushed before this final documentation acceptance.

Existing loader evidence still ends at syscall 9 with ten preceding Linux
returns. No new guest mapping, `true` exit or Antimatter execution is claimed.

## Next integration handoff

First split `linux_loader_entry_runtime::{run, run_owned}` into preparation,
guest execution and teardown. Preparation and teardown take separate lexical
`with_frame_allocator` borrows; guest execution must hold neither. Preserve
bootstrap-root restoration before cleanup and never store an allocator pointer
across entry. Qualify that lifetime change before adding publication.

Then bind preparation to the exact pending sequence, caller generation, saved
frame and retained source using `owned_execution::suspended_caller_index` and
`read_loader_source`. Source cleanup must follow runtime mapping withdrawal and
cleanup. The host tests here retain a source borrow but do not qualify a live
descriptor close/reopen or suspended-caller transition.

Introduce combined original/runtime ownership before splicing a root. The
existing `prepared_launch::committed_mapping_mut` contract prohibits payload or
tree modification. `owned_mapping::verify`,
`user_copy::validate_loader_suspended_copy_binding` and
`circle_table::new_multi_region` must agree on the combined layout. A spare root
entry alone does not establish those invariants.

Finally bind publication to `linux_loader_entry_runtime::complete_boundary` and
retirement to `owned_execution::finish`. Uncertain publication must retain a
blocked caller and its owner. Withdrawal must cover the caller's external tree
references and cached translations before candidate/source cleanup; an inactive
isolated staging root is insufficient. Fresh emitted review and live failure
qualification belong to that integration, not this dormant mechanism.
