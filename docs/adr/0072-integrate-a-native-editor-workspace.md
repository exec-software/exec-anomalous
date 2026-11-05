# ADR 0072: Integrate a native editor workspace

- Status: Proposed; bounded native integration qualification complete
- Date: 2026-10-07
- Related: ADR 0045, ADR 0048, ADR 0063, ADR 0065, ADR 0067, ADR 0068, ADR 0070, ADR 0071

## Context

ADR 0070 establishes one native owner for three heterogeneous task layouts, but
its small assembly probes do not run the applications. ADR 0071 establishes
copied editor actions, completions and observations against the actual editor,
durable store and compositor on the host. The remaining boundary is to connect
that trusted receive adapter to installed native routes and execute the real
workspace, editor session and document service in separate address spaces.

The earlier interactive pair cannot be linked unchanged. Its editor owns
physical input and presentation, and its service reads UI observations from
pair route zero to release a delayed reply. In the three-owner layout those
operations belong to the workspace, and route zero carries workspace actions.
The service must not gain access to that route to preserve a proof script.

## Decision

Select an opt-in `native_editor_workspace_test` profile. It replaces the mixed
admission probe runtime at compile time with a single application coordinator;
it does not activate the old pair and workspace runtimes simultaneously. Build
the workspace with `native_remote` and the shared editor/service payload with
`remote_workspace_proof`. Package the workspace RX/data and shared application
RX/data once each, then copy the application image into two independent owners.
The separate roles do not share writable state merely because their executable
bytes originate in one build artifact.

This is a fixed native Rust reference for Antimatter's service contract. The
selected launch opens one native editor window in the actual desktop and uses
the actual interactive editor and durable store. It is not the complete
Antimatter Electron/React/CodeMirror application, a freestanding Ny runtime, a
HAG implementation or a general executable launcher. A stable Reliquary `.re`
container is still separate; native execution here does not claim ELF or Linux
compatibility.

### Retain one owner for dispatch, roots and CPU state

Keep ADR 0070's trusted role order: workspace zero, editor one, service two.
The selected coordinator retains all task and channel identities, page-table
roots, transition stacks, scheduler contexts and three eager CPU images. Integer
entry preflight establishes current frame ownership before capturing user CPU
state and installing the clean kernel image. Gate and user IRQ return restore
only the selected owner's state. Cooperative yield chooses another runnable
owner; returning timer and physical-input interrupts preserve ownership without
introducing a second active scheduler or CPU session.

Preserve the admitted geometry and guards from ADR 0070:

| Owner | RX | Ordinary RW/NX data | Private arena | User stack |
| --- | ---: | ---: | ---: | ---: |
| Workspace | 128 KiB | 32 KiB | 36,864 bytes at `0x810000` | 64 KiB |
| Editor | 32 KiB | 8 KiB | 32 KiB at `0x804000` | 8 KiB |
| Service | 32 KiB | 8 KiB | 32 KiB at `0x814000` | 8 KiB |

The workspace mailbox remains at `0x800000`; its copied startup descriptor and
linked data start at `0x801000`. Editor/service descriptors occupy the same
virtual address in their own roots. Populate admitted RX/data from inspected
artifacts, zero the remaining owned bytes, and verify the private arenas before
entry. Identical virtual addresses across tasks do not identify the same frames.

The three task owners require 144 frames in 18 leases, including 27 table frames
and 12 transition-stack frames. Full-screen presentation adds a separate
576-frame lease for the 1024 by 768 RGB stage. The designed live total is
720 frames in 19 leases: 693 data frames and 27 table frames. These are admission
and accounting requirements, not measured emitted stack bounds. The final
workspace/application artifacts and kernel gate, IRQ, initialization and cleanup
paths require fresh inspection. Linker limits and successful compilation alone
cannot prove those bounds or the fixed Threshold kernel-image capacity.

### Install four directed routes and narrow device rights

Use the existing one-packet copied channel mechanism. Its private binding
authenticates the current task and endpoint incarnation before checking a user
copy or mutating a queue. The four fixture handles are:

| Handle | Installed direction | Complete copied packet |
| --- | --- | --- |
| 1 | Workspace to editor | ANEA, 64 bytes |
| 2 | Editor to workspace | ANER, 192 bytes |
| 3 | Editor to service | ANDC, or qualification-only ANDH, 128 bytes |
| 4 | Service to editor | Mount readiness or ANDC, 128 bytes |

Each direction has its own consecutive transport sequence. An ANDH marker
consumes a transport sequence without reserving a document request ID. Transport
delivery and acknowledgement never prove editor completion, presentation or a
durable save. Packet codecs and numeric handle values cannot grant a route.

The workspace alone possesses the `f3` mailbox rights for physical input,
complete desktop row composition, presentation, yield and exit. Its existing
desktop focus, modal, input-loss and grant policy governs native actions. The
editor and service cannot use `f3`. No task retains the old pair's `f4` input or
display operations. All owners use `f4` startup, role-checked copied transport,
yield and terminal exit.

Only service role two has the exact current storage handle. Its `f4` operations
12, 13 and 14 admit sector read, write and flush under the retained controller
and disposable extent. Reads and writes require exactly 512 bytes and a relative
sector in zero through four; flush has no buffer or sector operands. Check role
and handle before user arguments or device effects, preflight a complete read
destination before I/O, and copy a complete write before I/O. Workspace/editor
guesses of the service handle are denied. Disk format, mount, checkpoint and
save policy remain in the userspace service under ADR 0063.

### Copy startup metadata without transferring authority

Add opt-in `anomalous-document-protocol/remote_workspace`. Its experimental
ANEW version-one descriptor is exactly 192 bytes, with explicit little-endian
encoding, strict role/phase/extent/handle validation and zero reserved bytes.
The kernel copies it into each admitted owner and supplies its address and size
through the existing startup entry contract.

| Offset | Field |
| --- | --- |
| 0 | `ANEW`, version 1, length 192 |
| 8, 16 | Role; phase 1 for format or 2 for recovery |
| 24, 32 | This role's private arena base and length |
| 40 through 64 | Four route handles |
| 72, 80 | Installed editor task and generation correlation |
| 88, 96 | Durable volume and object identity |
| 104, 112 | Service-only device handle; qualification mode |
| 120 | Workspace/editor connection correlation |
| 128, 136 | Installed workspace task and generation correlation |
| 144, 152 | Action and observation endpoint generations |
| 160 through 191 | Zero reserved bytes |

Startup contains no document text, injected edit or expected recovery answer.
The kernel supplies identities from retained owners independently of application
packets. The workspace constructs its local port binding from that startup and
invokes the ADR 0071 checked-receipt API only after a successful complete native
receive on the installed observation route. The editor similarly calls its
execution adapter only after a checked action-route receive. ANEW describes
these grants; modifying a descriptor cannot create them.

### Keep editor and presentation ownership separate

The editor alone owns one `remote_editor_session` and its retained save
snapshot. It opens the document after service mount readiness, emits its initial
ANER observation, then processes copied actions and asynchronous document
replies. The workspace contains observations and publication correlation, with
no mutable shadow editor, save snapshot or document authority.

Preserve ADR 0071's exact action fence. The workspace publishes an authenticated
initial observation before admitting input. A submitted action retains origin,
action ID and expected editor sequence; unsolicited observations and transport
ACKs cannot release its fence. Keep receiving replies while fenced. An exact
completion permits publication of the latest authenticated observation and
subsequent input. A stale completion has no editor effect and cannot roll back
a newer service-driven observation.

Render into exactly two borrowed 18,432-byte slots in the workspace arena. Frame
metadata remains outside those slots. The displayed and admitted pending frame
stay immutable; successful publication recycles the former displayed slot.
Complete desktop rows compose that native surface with the existing desktop,
then use the separately retained full-screen stage. This establishes neither
scanout completion nor a GPU fence. Host-side pixel expectations must check both
the editor content and its placement in the composed desktop.

The editor sends and obtains transport acknowledgement of a `save_reserved`
completion before submitting the corresponding ANDC save. Reservation is only
local completion. An exact later service reply advances the saved baseline;
acknowledging B after an edit to C must leave C dirty. A service cut marks an
awaiting save unknown while keeping its snapshot. Workspace/editor routes and
local editing remain live; service loss cannot confirm close or clear a desktop
action fence.

### Release delayed proof replies on the document route

Qualification mode zero uses ordinary replies. Mode one holds the first
successfully committed save reply until the editor reports a later local text
revision. Mode two retires the service after that first save commits and before
the reply is sent. These modes provision failure/delay behavior only; they do
not supply editor actions or expected document contents.

Mode one's release uses a separate experimental ANDH version-one record, exactly
128 bytes. Its six little-endian words at offsets 8 through 48 are document,
open generation, request, base version, submitted revision and later revision;
bytes 56 through 127 must be zero. The later revision must exceed the submitted
revision. The service matches the complete pending saved tuple before releasing
its retained ANDC reply. The marker is admitted only on the authenticated
editor-to-service route. It is not a durable acknowledgement, a new save, an
ANDC operation or an assertion of authority.

The editor queues this marker after the later action completion without blocking
on the marker ACK, so it can drain a concurrent definitive rejection. The
service may consume one exact matching marker after a first-save rejection as
a correlation-only no-op. Neither case changes document request IDs or store
state. Duplicate, malformed or unrelated markers do not release a held reply.
The service never reads workspace actions or UI observations.

### Retire through the same coordinator

Terminal retirement closes and clears only routes incident to that exact task,
records discarded queued packets, and removes the task from dispatch. Service
retirement leaves both workspace/editor routes available. Workspace retirement
causes the editor to abandon its local ownership and exit; editor retirement
causes the service to exit when its document route closes. An exact closed
completion is acknowledged before editor exit, and the workspace publishes its
accepted closed state before returning terminally.

If the editor withdraws without a closed completion, the workspace uses the
trusted terminal route result to invalidate its port and bridge before lifting
a matching fence and removing the window. This does not manufacture save success
or a confirmed close. A terminal user fault is attributable to its current owner;
successful qualification must distinguish expected orderly cuts from faults.

Restore the bootstrap root, TSS, CPU controls, CPU image and continuation before
releasing owners. Withdraw terminal contexts, revoke identities, clear/release
the task leases and presentation-stage lease, and compare allocator, scheduler,
task and circle state with the recorded baseline. Keep failed cleanup attributable
instead of dropping retained owners or silently continuing another profile.

## Qualification

The final native matrix `matrix-v2`, retained as
`native-editor-workspace-proof-6wzvzkjj`, passed sixteen initial/cold boots:
TCG and KVM, each at 96 and 320 MiB, with held-reply mode one and committed-save
service-loss mode two. Physical PS/2 keyboard and pointer traces exercise the
composed window, B then C edits, delayed B acknowledgement with dirty C, later
clean save, unknown save after service loss, continued editing, close and exact
resource restoration. Cold recovery uses the same disposable media and supplies
no expected document text through startup.

The matrix checked 140 complete desktop frames comprising 107,520 rows and 96
stable screenshot comparisons. It recorded 208 input events with zero loss,
176 expected role denials and 48 orderly task exits, with zero task faults or
bad-copy events. Literal packet/media expectations and composed pixel checks
remain distinct evidence; transport acknowledgement alone establishes none of
these application outcomes.

Final emitted review binds these artifacts:

| Artifact | SHA-256 |
| --- | --- |
| Kernel ELF | `590f39ec45a310392d062af1aa629a9e97742ab46f34247aaabe0e05038a3497` |
| Workspace ELF | `2f5aa6d3bfa61dc13259a1056a20513842a58ebca31c273c480aa1b7c134a137` |
| Shared editor/service ELF | `91525f16f278c70f3a9edf183079b9250a027f392dba30529af9cade2aba6eb2` |
| Boot image | `0d0284832ef85a58c260c341aaa3105c84d68abfa837a6b7923d25e9d5b49acf` |

The workspace contains 102,868 bytes RX and 208 bytes initialized data; the
shared application contains 23,760 bytes RX and 4,288 bytes initialized data.
The flat kernel is 294,912 bytes within the unchanged 393,216-byte Threshold
capacity. Conservative emitted stack/control review establishes:

| Path | Bound in bytes | Available bytes |
| --- | ---: | ---: |
| Workspace user stack | 26,704 | 65,536 |
| Editor/service user stack | 4,816 | 8,192 |
| Privilege gate plus one idle IRQ | 6,230 | 16,384 |
| Bootstrap plus one IRQ | 32,375 | 65,536 |

These are emitted bounds, not runtime high-water measurements. The independent
CPU review checks integer-only entry/return paths, eight explicit FX image
instructions, disjoint retained CPU images, root/TSS ordering and terminal
restoration. The admitted reviews exclude fatal diagnostic paths, arbitrary
nested exceptions/NMI and faults during CPU save/restore. After the host-only
verification corrections at `8bc98bb`, all four artifacts reproduced
byte-for-byte; kernel, vessel and shared-library source were unchanged.

Both isolated VirtualBox 7.2.6 attempts, including the final image, stopped at
`interactive_fixture_signature`: this profile expects the QEMU fw_cfg fixture.
It did not reach native task startup. That attempt is a fixture failure, not
an observed CPU-admission refusal or successful application qualification; no
CPU policy was weakened.

All sixteen required checks (`required-v2`), affected mixed-admission,
interactive-save/recovery, owned-CPU and 512-frame desktop regressions passed.
All four rebuilt baseline images are byte-identical. The final audit binds
447 unchanged source/configuration files, exact emitted and guest receipts,
and all 22 guarded scopes including failed attempts. No OOM counter increase,
pressure abort or attributable journal kill was observed. The overall peak was
4.0015 GiB with 3,041 soft-limit reclaim events in an earlier failed check;
the successful required suite peaked at 2.94 GiB. Accessible journals are partial
and missing unit counters remain unavailable, not zero. Focused codec tests,
warnings-denied checks, packaging and normal exclusion results are retained in the
[qualification log](../../notebook/Development/2026-10-07%20Native%20Editor%20Workspace%20Log.md).
Continue serializing builds and VMs with one build job under 4 GiB `memory.high`,
6 GiB `memory.max` and 512 MiB scope swap limits, with host/ancestor pressure
monitoring. Broader Antimatter runtime support and the native/ELF behavioral
comparison remain separate milestones.
