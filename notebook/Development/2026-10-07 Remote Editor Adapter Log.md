# Remote Editor Adapter Log — 2026-10-07

## Scope

Continues from pushed `d54bd74d7bd36244e020f3eacb8ab0cce3af8d9b`.
The three-task admission proof is complete; this slice replaces direct editor
borrows with an opt-in copied protocol and checked workspace receipt seam.
It does not select new guest payloads. See
[ADR 0071](../../docs/adr/0071-compose-a-copied-remote-editor-adapter.md).

The source audit found that the old interactive service assumes pair route zero
contains editor ANUI, whereas the mixed runtime's route zero carries workspace
actions. Its proof delay/termination behavior must change before real payload
selection. Workspace physical focus routing and editor input/display ownership
also need explicit adaptation. These are concrete next integration changes,
not reasons to grant the service unrelated action authority.

## Evidence and memory policy

Evidence directory:
an external validation directory (not distributed).
`turn-start.json` records the starting commit and unchanged guard SHA-256;
`baseline-images.json` records the four starting boot images. The guard's pure
self-check passed before any build.

Only the primary agent runs builds and VMs. Jobs are serialized with
`CARGO_BUILD_JOBS=1` and `RUST_TEST_THREADS=1`, cgroup MemoryHigh 4 GiB,
MemoryMax 6 GiB and MemorySwapMax 512 MiB. One-second samples cover host memory
and PSI plus the scope and every cgroup ancestor. Each job requires 30 seconds
of continuously quiet admission. The guard stops only its own process group
for low available memory, observed OOM, sustained pressure or stale monitoring.
It does not change host OOM policy or touch unrelated VMs/processes.

## Focused checkpoint

`focused-v1` passed formatting, tests and warnings-denied host/freestanding
Clippy for all three opt-in crates: editor contract `remote_editor`, workspace
and compositor `native_editor_remote`. The codec adds nine focused test groups,
the workspace adds seven and the compositor adds three multi-stage integration
tests. Existing tests also passed: totals were 52 editor unit tests, 103 workspace
unit tests plus 15 documentation tests, and 62 compositor unit tests plus 13
integration tests across the three bridge/coordinator/remote suites.

The first guarded scope finished successfully with a 575,881,216-byte sampled
scope peak, 22,010,609,664-byte minimum available host memory, no scope high/max
or OOM events and no kernel OOM counter increase. Full matrix and journal/image
qualification follow this tested implementation checkpoint.


## Frozen-source qualification

The tested implementation checkpoint is `82ff1d9`. `source-freeze.json` records
SHA-256 for all 436 tracked non-Markdown source/configuration files before the
full qualification. Later documentation edits do not change that candidate.
An independent read-only review found no additional actionable defect in the
combined codec/workspace authority, overflow, stale result or retirement paths.

All sixteen required targets passed in `required-v1`, including the expanded
`make check` coverage for the new opt-in features. The semantic presentation
proof matched the independent 1024×768 oracle and restored all retained frames.
The required scope peaked at 3,443,527,680 bytes (3.207 GiB); minimum sampled host
availability was 21,105,283,072 bytes (19.656 GiB). It recorded no high/max events,
OOM counter increase, pressure abort or failed job. The desktop regression,
image comparison and combined journal audit completed as recorded below.


## Final regression and evidence audit

`regression-v1` passed the existing workspace regression: 21 exact keyboard,
465 pointer/Appearance and 26 graphics frames, totaling 512 complete frames,
including orderly exit, contained fault, watchdog, malformed context/mailbox,
resource restoration and shell fallback. It then rebuilt the normal, workspace,
owned-workspace and eager-CPU owned-workspace images. All four match the starting
SHA-256 values exactly:

| Image | SHA-256 |
| --- | --- |
| `build/anomalous.img` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` |
| `build/workspace-dev.img` | `8aa7f44f3f3a444e8b5c2124091e39408c4bc1c1c73087719d98c6f236d71408` |
| `build/owned-workspace-test.img` | `7f819e6df5969c4894919215e9fd48f3b6d93743b03505b806ab6eeaa4fc4554` |
| `build/owned-workspace-cpu-test.img` | `cc2b16108fac639bb54014fd241837d1a34d35ccf53f11f85e8fa395ac70e0a4` |

No guest boot code or selected payload changed, so this slice did not repeat the
prior VirtualBox CPU-capability refusal or qualify the new adapter there. The
existing native images and their prior qualification remain distinct from the
new copied host composition.

All 28 guarded jobs across three scopes passed. Their complete final summaries,
raw samples, commands, logs and the accessible unit journals are bound by
`final-audit/receipt.json`. The audit verifies no job overlap, all sixteen exact
required commands, unchanged frozen source, the 512-frame completion and all four
image comparisons. All scopes recorded zero high/max/OOM events, zero host OOM
counter increase and no sampled pressure threshold breach or guard abort. The
maximum scope peak was 3,443,527,680 bytes; minimum available host memory was
21,105,283,072 bytes.

The accessible user journals show normal scope starts and completion/resource
reports, without a kill report. Collected transient units no longer expose
historical OOM/oomd counters; those values are unavailable, not inferred zeros.
The systemd-oomd query returned no accessible records. This is no observed kill
with complete runner receipts and partial journal coverage, not a claim of
complete host telemetry.

Receipt SHA-256:
`dcb7e58dcadd61fdc01a5b1c23db45ee8bc5e7f3d5f5057526c88563cf1a16d4`.


## Next native slice

Select the real workspace, sole editor and durable service under the single
mixed runtime. Replace the pair's route-zero ANUI/delay assumptions, bind checked
receive provenance to installed task/channel incarnations, assign input/display
to workspace and disk operations to service, and route focused input through the
copied adapter. Retain the two mapped frame slots and qualify final image capacity,
user/kernel emitted stacks, physical input, composed pixels, delayed saves,
service loss and cold recovery. The full Antimatter/Electron port remains open;
this checkpoint completes its copied reference-editor prerequisite only.
