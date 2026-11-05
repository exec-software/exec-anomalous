# Native window content surfaces

Date: 2026-09-26

## Scope and current status

The user accepted the mouse integration and requested graphics and rendering
inside native windows as a foundation for TRI-ABI development, starting with
Linux. The selected slice is a reusable bounded software surface path and a
native Graphics Lab demonstrator. Filesystem and file-app work remain outside
this request.

The implementation under proposed
[ADR 0038](../../docs/adr/0038-add-native-window-content-surfaces.md) passed all
20 repository validation targets and an additional complete KVM workspace
smoke. Both workspace runs verified the new 23-frame graphics sequence plus the
retained 21 keyboard and 410 pointer/Appearance frames, with complete resource
restoration and working shell fallback. Native timing, a sustained-input exact
capture, and the separate Graphics Lab interactive launch also passed. These
results apply to this increment's frozen final source and image.

## Implemented contract

The portable `anomalous-window-surface` crate validates opaque RGB8 layouts with
nonzero dimensions up to 1024×768, stride no greater than 4096 bytes, and exact
`stride * height` backing length. It exposes immutable borrowed frames with
process-local unique nonwrapping identities and copyable public metadata,
including a printable ASCII label of 1–64 bytes. Caller-owned drawing covers clear,
filled rectangles, lines, and a deterministic demonstration scene. Pixels stay
outside the canonical desktop snapshot.

A canonical window records the exact descriptor and `surface_sequence`, shared
by human, accessibility, and machine observations. The distinct `present` grant
and a move-only owner bind replacement to an exact client generation and window
incarnation. Preparation uses the exact current desktop revision and the
existing one-successor limit. Initial window creation and initial descriptor
publish together; replacement remains gated by the full-frame acknowledgement.

The compositor validates every retained immutable binding against the complete
candidate before presentation begins. The bounded `composition_frame` requires
all attached live windows, including minimized and other-workspace windows,
and rejects missing, duplicate, extraneous, or mismatched bindings.
It then samples those same frames inside
the public client viewport, preserving window chrome, z-order, clipping, and
the pointer. Nearest-neighbor stretching lets existing resize and maximize
operations work without changing or reallocating source pixels. This is not an
application resize-event protocol.

Graphics Lab runs inside the existing trusted ring-3 workspace vessel. **F2**
opens/restores the singleton; **F3** changes the demonstration scene through the
content-update path when Graphics Lab is focused. Startup draws two 96×64 RGB8
caller-owned buffers once, then retains both immutable frames: an orange/teal
geometry scene over a gradient and a five-bar chart with a rising line. F3
alternates those frames without redrawing their bytes; each successful update
advances the canonical content sequence. F2 retains the selected scene when
restoring the existing window. This is a local producer/compositor
contract, not independent application processes, shared-memory transport,
foreign execution, GPU rendering, or a stable graphics ABI.

The published label, geometry, and content sequence make the fixed demo
inspectable. They do not expose a semantic tree for arbitrary raster UI. Private
content, application input, accessibility semantics beyond this public demo,
authenticated external producers, and service-loss recovery remain separate
work.

## Linux-first compatibility direction

Linux is the first intended TRI-ABI experiment under the independent veil
approach of [ADR 0008](../../docs/adr/0008-isolate-foreign-platform-compatibility.md).
Linux-first defines implementation order. macOS has its own Darwin/XNU
foundation, combining Mach and BSD components, while Linux has a separate
kernel and ABI. [Apple's XNU README](https://github.com/apple-oss-distributions/xnu)
provides the primary platform reference. Each veil needs independent execution
and graphics contracts even when adapters can share native rendering facilities.

A future Linux path needs an explicit executable/runtime tier in addition to a
graphics bridge. Wayland's official protocol separates shared-memory buffer
layout and surface commit/release behavior; it does not itself provide a Linux
ELF loader or syscall implementation. The
[Wayland specification](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_shm)
informs that separation. This slice implements no Wayland, X11, Linux syscall,
macOS, or Windows compatibility. Which execution tier follows remains a later
user-directed decision.

## Component evidence

The complete `make check` passed in **244.3 seconds**, including formatting,
warning-denied Clippy, host tests, compile-fail documentation tests, and script
checks. The focused counts are:

| Component | Passing tests |
| --- | ---: |
| Window surface layout, raster drawing, and frame identity | 12 unit + 1 compile-fail |
| Canonical desktop workspace | 41 unit + 10 compile-fail |
| Desktop compositor | 50 unit |
| Native input adapters | 42 unit |
| Standalone vessel mailbox copy | 2 unit |
| `xtask` | 83 unit |

Surface cases cover dimensions, exact lengths, stride padding, bounded drawing,
extreme coordinates, immutable borrowing, identity, and exhaustion. Workspace
cases cover presentation grants, exact client/window identity, stale revision,
backpressure, cancellation, shared metadata, and sequence exhaustion. The eight
new compositor surface tests check binding admission, independently calculated
sampling and clipping, opacity and occlusion, hidden/minimized windows, retained
content, and independent window bindings. `xtask` includes the Graphics Lab
command-sequence oracle. The complete native lifecycle matrix also passed,
with the per-target results recorded below.

The two demo buffers occupy 36,864 bytes on the existing 64 KiB user stack.
The measured fixed stack allocation for the active run/presentation path is
49,072 bytes, leaving 16,464 bytes before deeper helper frames. This is a
bounded profile inspection, not a general stack-depth guarantee. The native
owner retains the same 16-page stack profile; no general application memory
allocation or separate producer task was added.

## Native responsiveness and exact retained content

The measured workspace image SHA-256 is
`7ef45ff45e6592ee1399f8922e95a87dc946afcbdb19004464a445e2d1275e9d`.
All scenes below use that image under KVM, with 24 isolated mouse motions per
scene. Times measure host input-command submission through observation of the
next complete native frame. They include QEMU delivery, guest work, host
scheduling, and serial observation; they are not physical input-to-scanout
measurements.

| Scene | Median, ms | P95, ms |
| --- | ---: | ---: |
| Default desktop | 8.77 | 9.16 |
| Graphics Lab geometry | 9.45 | 10.57 |
| Graphics Lab chart | 9.48 | 10.48 |
| Maximized Graphics Lab chart | 10.27 | 11.18 |

A separate maximized-chart sweep injects 200 four-pixel movements at 100 Hz
across a nominal two-second interval, with all 200 commands acknowledged.
Ready-motion batching produces 165 complete frames; intermediate physical
positions do not each require publication. The median observed frame interval
is 10.53 ms, and the largest interval is 57.31 ms. The last frame arrives
10.81 ms after the full injection interval and 21.10 ms after the final
injection. Host load is not isolated, so these measurements establish this run
rather than a guaranteed frame rate or latency ceiling.

The final native capture is revision 169 with pointer (512, 384). All
786,432 pixels and 2,359,296 RGB bytes match the retained host reference exactly.
The native and reference PPM SHA-256 is
`b1f5a5af6078389a6f02994a2ff1bdb173381d308e9c6ea34fed3f0a25573f79`.
The reference drives the canonical model directly and shares the compositor;
it is independent of the native physical-input adapter and guest framebuffer
transport, not an independent rendering implementation. The separate compositor
unit tests supply independent sampling and geometry expectations.

Local task evidence is retained in
`work/surface-validation/initial/measurements.json`,
`work/surface-validation/maximized-100hz/measurements.json`, and its `final.ppm`.
The comparison and reference source are
`work/surface-oracle/result.json` and `work/surface-oracle/src/main.rs`.

## Native artifact and interactive session

A private copy of the measured image was launched as
**Anomalous Workspace - Graphics Lab**, with Graphics Lab opened through F2.
The verified session used QEMU PID 2408405 and
`build/desktop-runs/run-graphics-2408403/workspace-dev.img`.
Its private image hash matched the measured image above. The native readiness
record reports 18 image pages, eight data pages, and sixteen stack pages.
The first two complete 768-row frames, active KVM, and zero stderr bytes were
verified in `work/surface-validation/live-session.json`.

That interactive VM subsequently exited, and its launch helper cleaned up the
private disk. Its retained bounded serial log records more than 500 complete
frames and zero stderr bytes. The PID and private path above are historical
launch evidence, not a currently available session or artifact. Earlier preview
VMs also no longer appear live; this task did not terminate them. The reusable
`build/workspace-dev.img` remains available, and `make workspace-run` starts a
fresh session when needed.

## Final regression evidence

All 20 repository validation targets passed on the frozen final source. The
ordinary proof runners retained TCG; a separate complete KVM workspace smoke
also passed. The 211 entries in `work/surface-validation/source-sha256.json`
remained unchanged through the runs, and the rebuilt image matches the measured
and interactive artifact above.

Each workspace run verified **23 exact graphics frames**, in addition to the
retained **21 keyboard** and **410 pointer/Appearance frames**. The graphics
sequence covers opening, repeated immutable scene replacement, move/resize,
maximize/restore, all three layouts, minimize/restore, workspace switching and
retrieval, theme changes, close/reopen identity, and retained content. Canonical
host projections also agree on descriptor, label, geometry, and content
sequence. This is shared local-model evidence, not a native external semantic
IPC implementation.

Both runs preserve controlled fault, watchdog withdrawal, malformed context,
malformed mailbox, orderly exit, exact resource restoration, and working shell
fallback. The strict transcript and exact captures use independently selected
commands and a host canonical model/compositor reference. As with the sustained
capture, shared rendering code proves cross-build presentation and transport;
independent compositor pixel tests supplement it.

The TCG workspace smoke passed in **99.1 seconds** and the KVM repeat in
**53.3 seconds**. The durations below sum to **1023.8 seconds**, excluding
exploration and earlier component runs; this is summed target time rather than
total wall time. Logs and results are retained in
`work/surface-validation/results.tsv`, `runner.log`, and the individual target
logs. Graphics captures and their native transcript are in
`build/workspace-graphics-smoke/workspace_initial.ppm`,
`workspace_customized.ppm`, and `workspace_smoke.log`.

| Target | Result | Seconds |
| --- | --- | ---: |
| `make check` | Passed | 244.3 |
| `make smoke` | Passed | 0.4 |
| `make fault-smoke` | Passed | 0.4 |
| `make circle-smoke` | Passed | 32.5 |
| `make control-smoke` | Passed | 32.1 |
| `make rite-smoke` | Passed | 32.6 |
| `make scheduler-smoke` | Passed | 33.6 |
| `make supervision-smoke` | Passed | 33.6 |
| `make shell-vessel-smoke` | Passed | 33.0 |
| `make memory-smoke` | Passed | 32.1 |
| `make interrupt-smoke` | Passed | 32.5 |
| `make input-wait-smoke` | Passed | 31.7 |
| `make shell-smoke` | Passed | 0.6 |
| `make display-vessel-smoke` | Passed | 33.9 |
| `make supervised-renderer-smoke` | Passed | 34.1 |
| `make semantic-surface-transport-smoke` | Passed | 83.3 |
| `make semantic-presentation-smoke` | Passed | 81.2 |
| `make desktop-smoke` | Passed | 65.7 |
| `make workspace-smoke` | Passed | 99.1 |
| `make normal-exclusion` | Passed | 33.8 |
| `make workspace-smoke` (KVM repeat) | Passed | 53.3 |

The final artifact check also matched the smoke's exact initial graphics PPM
to the retained interactive F2 startup capture. Normal-image exclusion passed;
accepted fixture ADRs remain unchanged. These results do not claim foreign-ABI
support, application-process isolation, GPU
composition, scanout completion, or a production desktop runtime. VirtualBox
was not rerun for this graphics increment.
