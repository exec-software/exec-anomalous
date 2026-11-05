# Native mouse responsiveness

Date: 2026-09-26

## Scope and current status

The user reported a slow, lagging pointer after capturing the VM and requested
that mouse integration be fixed before deciding the next development direction.
This follow-up measures and reduces the existing complete-frame cost. It does
not begin filesystem, file-app, or dispatcher work; the user has plans for those
later slices.

Rendering/transfer optimizations and their component checks are complete.
The ready-motion batching follow-up has passed its adapter checks under
[proposed ADR 0037](../../docs/adr/0037-batch-ready-desktop-pointer-motion.md).
The first regression matrix was stopped after the scheduler target when this
changed the code. Final native timing and exact captured-frame comparisons
have passed. The fresh complete 20-target matrix and an additional full KVM
workspace smoke also passed. The final image has been launched separately as **Anomalous Workspace - Mouse Integration** with KVM
verified active. Earlier full passes and pre-batching measurements do not
validate this final increment; older sessions remain preserved.

## Final native timing and sustained motion

The final image is
`426d5312fd9484d13b388428dc16c5e8a5454acb3c4af38159e8c4d94379a738`.
Measurements use private headless QEMU instances, acknowledged QMP input
commands, and concurrent incremental serial observation. Each isolated-input
scene has 24 samples. The host observes command submission through the next
complete-frame event; the polling interval is approximately 0.5 ms.

| Backend | Scene | Median, ms | P95, ms |
| --- | --- | ---: | ---: |
| KVM | Default desktop | 8.9 | 9.6 |
| KVM | Appearance open | 11.2 | 12.3 |
| TCG | Default desktop | 51.1 | 51.6 |
| TCG | Appearance open | 68.8 | 71.1 |

The original image measured 163.5/245.1 ms under TCG and 19.5/29.4 ms under
KVM for desktop/Appearance. Original isolated samples used the earlier HMP
probe; short acknowledged QMP repeats reproduced the burst timing closely.
These comparisons show reduced guest work under either backend as well as the
benefit of KVM for the interactive launch. The software fallback still presents
individual frames less quickly.

Five final sustained-input runs each inject 200 movements at 100 Hz with
Appearance open. The table's tail is the time from the end of the full nominal
two-second input interval to the last observed complete frame, floored at zero.
The final command is sent roughly 10 ms before that interval ends, so the
separate last-command tail is also shown. A zero interval tail does not mean
zero input latency.

| Backend and motion | Tail after interval, ms | Tail after final command, ms | Final cursor | Exact full-frame comparison |
| --- | ---: | ---: | --- | --- |
| TCG, positive four-pixel steps | 98.3 | 108.4 | (1023, 384) | Passed |
| TCG, horizontal sweep | 123.9 | 133.9 | (512, 384) | Passed |
| KVM, positive four-pixel steps | 0.0 | 9.4 | (1023, 384) | Passed |
| KVM, horizontal sweep | 2.8 | 12.8 | (512, 384) | Passed |
| TCG, positive one-pixel steps | 78.6 | 89.1 | (712, 384) | Passed |

All 1000 QMP commands were acknowledged within 1.7 ms. The unclamped one-pixel
run starts at (512, 384) and finishes at (712, 384), confirming all 200 units of
motion survive batching in that run. All five complete native PPM captures
match an independently driven canonical model/compositor oracle byte for byte,
including cursor, revision, and every UI pixel. The oracle does not call the
native pointer adapter; it shares the canonical renderer and therefore proves
cross-build presentation rather than an independent rendering algorithm.

Under the same sustained-input method, optimized pre-batching TCG needed
3434.4/3353.0 ms of interval tail for positive/sweep runs, compared with the
final 98.3/123.9 ms. The original TCG sweep needed 15657.3 ms. Calm pre-batching
KVM could already keep pace; a separate contended run developed an 845.6 ms
tail while its command acknowledgements stayed below 5 ms. QMP reproduced that
backlog, so interactive HMP echo was not its explanation. The bounded batching
policy addresses queued motion without assuming constant host performance.

Repository checks ran concurrently, and unrelated user workloads were left
untouched. Host load was not controlled or isolated. These are host-observed
results for the measured scenes, not physical mouse-to-scanout measurements or
a universal latency guarantee. The final source data, methods, hashes, and
individual comparison records are retained in
`work/mouse-sustained/final-summary.json`. All temporary probe VMs were
terminated; the interactive user VM was untouched. The separate full regression matrix
also passed, as recorded below.

## Pre-batching measurements and current interpretation

The original native workspace serialized each accepted pointer packet through
a full
1024×768 composition, copied row transfer, complete framebuffer presentation,
readback, and verified stage recycling. The previous interactive launcher used
QEMU's software emulation backend. Both the rendering cost and emulator choice
contributed to lag; increasing mouse sensitivity would not shorten this path.

The native probe injects 24 alternating four-unit horizontal mouse movements
for each scene. It measures from sending the monitor command until observing
the next `workspace_service.frame_presented` serial record, polling every
millisecond. The Appearance scene is opened through its ordinary keyboard
shortcut. This includes host scheduling, QEMU input delivery, guest work, and
serial observation. It does not measure a physical mouse, GUI redraw/scanout,
or pointer-to-photon latency, and it is not a throughput benchmark.

The baseline image SHA-256 is
`0875e2747350dcbe30bbe6bb993f7957e24b859432c2c1a7b91efc9b3247be9c`.
On this host, the same image produced:

| Backend | Scene | Median, ms | P95, ms |
| --- | --- | ---: | ---: |
| TCG | Default desktop | 163.5 | 169.1 |
| TCG | Appearance open | 245.1 | 257.0 |
| KVM | Default desktop | 19.5 | 19.9 |
| KVM | Appearance open | 29.4 | 34.3 |

The P95 column uses the 23rd ordered sample from each 24-sample run. Source
measurements and the probe are retained in the task's
`work/mouse-validation/before/measurements.json`,
`before-kvm/measurements.json`, and `probe.py`.

The optimized v1 image, before ready-motion batching, has SHA-256
`6203f603ab47e1e1ef4e24d0e2089bc5e3063b8ab404e26c81c061345b9fe676`.
The same 24-sample method on this pre-batching image gave:

| Backend | Scene | Median, ms | P95, ms |
| --- | --- | ---: | ---: |
| TCG | Default desktop | 49.6 | 52.1 |
| TCG | Appearance open | 69.7 | 70.6 |
| KVM | Default desktop | 8.1 | 8.8 |
| KVM | Appearance open | 10.4 | 12.3 |

This separates the guest improvements from the accelerator change: the
optimized image is faster under both backends. The software fallback still has
higher per-update latency. These are observations on this host and two bounded
scenes, not guarantees for arbitrary hardware, themes, or application loads.
The after measurements are in `work/mouse-validation/after-tcg/measurements.json`
and `after-kvm/measurements.json`.

A preliminary pre-batching Appearance burst probe sends 20 movements at
100 Hz and waits for all 20 complete frames. With the original image under TCG, completion lagged
behind the end of input by 4890.8 ms. With the optimized image under KVM, that
tail was 2.3 ms. Both runs presented all 20 frames. This comparison combines the
code and launch-policy changes; it does not isolate either improvement. Its
artifacts are `work/mouse-validation/burst-before/measurements.json`,
`burst-after/measurements.json`, and `burst.py`. These short-run results do
not establish sustained performance or validate the final batched path.

Longer streaming QMP experiments confirmed that backlog varies with host load:
calm and busy runs differ even when injection acknowledgements are prompt.
Interactive HMP echo can also complicate a long probe, but it does not explain
away all observed backlog. The final QMP comparisons above use concurrent serial observation and record
complete native captures. These preliminary runs remain historical evidence,
not a universal catch-up guarantee.

A separate host release benchmark times 20 full frames per scene with
`opt-level=s`, LTO, and one codegen unit. It excludes native transfer, emulator,
and file-I/O costs. Light desktop composition averaged 11.419 ms before and
1.901 ms after; Light with Appearance averaged 20.819 ms before and 4.019 ms
after. Dark and High Contrast were measured separately. These component
measurements establish reduced compositor cost, not final native latency.
The full results are retained in `work/mouse-profile/results.json`.

## Implementation and preserved invariants

The compositor fills certainly opaque interior spans directly and retains the
existing scalar coverage and blending at borders and antialiased corners.
Rounded outlines and shadow rings skip interiors where their contribution is
zero. Row-level bounds avoid evaluating panel, status, window, icon, pointer,
and text work that cannot affect the current output row. The paint order and
resolved theme state remain unchanged.

The vessel's private mailbox helper uses aligned volatile 64-bit accesses on
the mapped side. Ordinary source and destination slices need no word alignment.
Byte prefixes and tails cover exact unaligned edges. The kernel copies only
the admitted body after the fixed 40-byte header, rejects bodies over 4056
bytes before mutation, and clears all 4096 scratch bytes for every admitted
copy. The vessel remains stopped while the kernel copies its mailbox.

The framebuffer presenter uses aligned 32-bit writes and readback only when
the validated four-byte pixel layout and row addresses permit it. Other
layouts retain the byte path. Pitch padding, reserved pixel bits, channel
placement, and complete visible readback remain covered. Stage recycling and
teardown use aligned machine-word volatile clearing and verification with
exact byte prefixes and tails; no owned byte is skipped.

The rendering and transfer work stays within the existing ownership
boundaries. The ABI, mappings, granted actions, raw input sequences, accepted
geometry, 768-row completion count, and single pending semantic revision are
unchanged. No candidate is published before its complete frame is acknowledged.
Failure still withdraws the service and reclaims resources through the existing
path. The change in continuous-motion presentation cadence is recorded
separately in proposed ADR 0037; it adds no stable format or privilege boundary.

## Bounded ready-motion policy

The pointer adapter's `consume_ready` path incorporates at most 16 already-ready
continuous same-button packets, including the first, into one semantic update.
It validates every incorporated source generation and exact raw sequence. Each
relative delta is applied and viewport-clamped in order; summing deltas and
clamping once would mishandle motion beyond an edge followed by reversal.
Drag and resize retain the original capture anchor and live window incarnation.

The first packet needs an established generation and previous sequence, no
pending post-loss sequence-zero requirement, and the existing button mask.
Fresh sources and transitions remain singletons and do not poll. Entering or
leaving a chord is a boundary; motion with an unchanged chord can batch while
capture remains cancelled. Press, release, loss, keyboard input, and source
changes stay separate. At most one fetched boundary record is returned as
untouched lookahead for the next loop turn, and nothing is read beyond it. A
full 16-packet batch performs at most 15 additional polls and fetches no extra
lookahead at the cap. There is no intentional wait to collect input.

The adapter prepares state in a local copy. A read or validation error leaves
the original state unchanged and withdraws the owner; already-read records are
not retried. A successful batch still prepares one ordinary granted canonical
transaction, receives a complete-frame acknowledgement, and publishes one
shared revision. Intermediate continuous hover positions need not each get a
frame, while button/key behavior and cancellation rules remain intact.

## Interactive accelerator policy

Graphical Linux QEMU runs now default to `q35,accel=kvm:tcg`: KVM is attempted
first, and QEMU can fall back to TCG if KVM cannot initialize. Other hosts use
TCG in automatic mode. `ANOMALOUS_QEMU_ACCEL` accepts:

| Value | Interactive launch behavior |
| --- | --- |
| `auto` or unset | KVM with TCG fallback on Linux; TCG on other hosts |
| `kvm` | Require KVM; no software fallback |
| `tcg` | Require software emulation |

Invalid values are rejected explicitly. This selection applies to graphical
development launches, including the workspace. The established automated
proof runners keep their existing software-emulation backend; an additional
KVM workspace run provides separate evidence for the interactive path. Each
live VM still receives a private disk copy and bounded private diagnostics.

## Component evidence

The compositor's 42 tests passed, including equivalence against the previous
scalar paint calculation. Six complete Light, Dark, and High Contrast RGB
outputs match their pre-optimization images byte for byte. Eighteen full-frame
references additionally cover row culling across small/large metrics, empty
and maximized layouts, Appearance, text sizes, and cursor sizes. These checks
guard pixel identity independently of timing.

Two standalone vessel helper tests cover every mailbox length from 0 through
4096, all eight source/destination alignments, word tails, and preservation of
adjacent bytes. Two kernel helper tests cover every admitted body length from
0 through 4056 at all eight source alignments, complete scratch clearing,
zero-length bodies, and invalid-size no-mutation. Freestanding vessel and
kernel workspace Clippy checks passed with warnings denied. The display tests
cover both word and byte paths, layout guards, corruption readback, and exact
unaligned stage clearing. The final complete check includes these paths.

The service adapter suite passed 42 tests, including eight new batching tests
for the packet bound, untouched boundary lookahead, no poll after a transition
or the cap, raw sequence/generation failures, sequential edge clamping,
original drag/resize anchors, and release semantics. Its warning-denied host
Clippy check passed. Five exact sustained-input captures supplement the paced
interaction proof, and the fresh complete repository run passed.

## Final validation

All 20 repository validation targets passed on the frozen final source, followed
by an additional complete workspace smoke using KVM. The normal proof runners
retained TCG. The final `make check` passed in 237.3 seconds.
The 21 source files in `work/mouse-final/source-sha256.json` remained unchanged
through the runs, and the rebuilt workspace image matches the final measured
and interactive image.

The TCG workspace smoke passed in 73.3 seconds and the KVM repeat in 50.9
seconds. Each verified all 21 exact keyboard frames and 410 pointer/Appearance frames: 124 retained
window/pointer frames, 213 Appearance frames, and 73 release-gesture frames.
Both preserve two desktop sessions, controlled fault, watchdog withdrawal,
malformed context/mailbox handling, orderly exit, complete resource restoration,
and working shell fallback. The paced oracle retains independently chosen
commands and exact cross-build pixel comparison. The five sustained-input
captures above separately exercise queued motion and cumulative displacement.

The durations below sum to **1011.7 seconds** for the final passing runs, including the KVM repeat. This is not total wall time and excludes
the earlier interrupted matrix and exploratory measurements. Results and logs
are retained in `work/mouse-final/results.tsv` and the adjacent target logs.

| Target | Result | Seconds |
| --- | --- | ---: |
| `make check` | Passed | 237.3 |
| `make smoke` | Passed | 0.4 |
| `make fault-smoke` | Passed | 0.4 |
| `make circle-smoke` | Passed | 32.0 |
| `make control-smoke` | Passed | 32.0 |
| `make rite-smoke` | Passed | 37.6 |
| `make scheduler-smoke` | Passed | 32.0 |
| `make supervision-smoke` | Passed | 35.5 |
| `make shell-vessel-smoke` | Passed | 35.6 |
| `make memory-smoke` | Passed | 34.1 |
| `make interrupt-smoke` | Passed | 32.8 |
| `make input-wait-smoke` | Passed | 34.1 |
| `make shell-smoke` | Passed | 0.6 |
| `make display-vessel-smoke` | Passed | 33.7 |
| `make supervised-renderer-smoke` | Passed | 36.2 |
| `make semantic-surface-transport-smoke` | Passed | 88.9 |
| `make semantic-presentation-smoke` | Passed | 95.4 |
| `make desktop-smoke` | Passed | 54.8 |
| `make workspace-smoke` | Passed | 73.3 |
| `make normal-exclusion` | Passed | 34.1 |
| `make workspace-smoke` (KVM repeat) | Passed | 50.9 |

## Final native artifact and interactive session

The inspected final workspace image SHA-256 is
`426d5312fd9484d13b388428dc16c5e8a5454acb3c4af38159e8c4d94379a738`.
A private copy is running as **Anomalous Workspace - Mouse Integration**.
QEMU PID 2106848 uses
`build/desktop-runs/run-mouse-final-2106839/workspace-dev.img`.
Its hash matches the final inspected image. Native service readiness, a
complete 768-row first frame, zero stderr bytes, and active KVM file descriptors
were verified and retained in `work/mouse-final/live-session.json`.

This launch uses the final batching implementation, independently of the older
pre-batching KVM preview and earlier desktop sessions. Existing VMs and their
session-local state were preserved. The launch and full regression evidence
refer to the same final image; readiness and the complete matrix were verified
separately.

VirtualBox has not been rerun. Its earlier KVM/AMD-V conflict is historical
evidence, not a new result for this change. GPU composition, partial-frame
updates, a hardware cursor, and USB absolute-pointer integration remain outside
this measured correction.
