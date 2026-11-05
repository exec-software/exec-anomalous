# ADR 0070: Qualify mixed native workspace admission

- Status: Proposed; implemented and qualified as a bounded three-task TCG/KVM admission proof
- Date: 2026-10-05
- Related: ADR 0048, ADR 0055, ADR 0067, ADR 0068, ADR 0069

## Context

The portable split coordinator now preserves action ownership and desktop
publication while editor replies are pending. Its next dependency is one native
owner of three heterogeneous tasks. The existing workspace constructor admits
one large workspace, while the native arena constructor admits two small
application tasks. Neither can safely stand in for the other. Combining two
active runtimes would also duplicate CPU capture, root restoration and terminal
cleanup responsibilities.

Qualify the combined execution mechanism with explicit native probes before
adding the remote editor adapter and a visible desktop entry. This gate runs
three actual isolated userspace tasks. The probes do not execute the desktop,
editor session or storage service and must not be described as those applications.

## Decision

Select a separate `mixed_workspace_test` feature and one coordinator. Keep all
existing profile constructors and default images separate. A trusted constructor
fixes indices zero, one and two as workspace, editor and document-service roles;
copied role numbers or mapping counts cannot select this layout.

| Role/region | Base | Pages | Permissions |
| --- | --- | --- | --- |
| Workspace instructions | `0x400000` | 32 | User RX |
| Workspace mailbox/private data | `0x800000` | 8 | User RW, NX |
| Workspace frame storage | `0x810000` | 9 | User RW, NX |
| Workspace stack | Existing user stack top minus 64 KiB | 16 | User RW, NX |
| Editor/service instructions | `0x400000` | 8 each | User RX |
| Editor/service data | `0x800000` | 2 each | User RW, NX |
| Editor private arena | `0x804000` | 8 | User RW, NX |
| Service private arena | `0x814000` | 8 | User RW, NX |
| Editor/service stacks | Existing user stack top minus 8 KiB | 2 each | User RW, NX |

The nine workspace frame pages have room for exactly two 18,432-byte pixel slots.
The current probes write patterns into this storage; they do not construct or
publish editor frames. Metadata belongs in the ordinary data region. Reserve
the complete absent gap from `0x808000` to `0x810000` and the page at `0x819000`.
This preserves both frame guards within the existing eight-reservation capacity.
Other region guards and
the low supervisor exclusion remain mandatory. These are fixed proof layouts,
not general application, filesystem-name or path limits.

The three-task owner total is 144 frames in 18 leases: 27 table frames,
12 transition-stack frames and 105 user frames. A future full-screen presentation
stage needs its own accounting and is not included here. Populate immutable
payloads, clear every remaining owned byte and read back before publication.
Retain all owners and partial cleanup progress on failure. Preserve the 4096-byte
checked-copy bound and derive each copy binding from the selected retained
profile, never from user claims.

One scheduler, selected-task identity, retained root/TSS owner and resident
three-image eager CPU session handle entry, dispatch and terminal restoration.
Validate complete frame provenance before policy. Restrict the entry preflight
to integer work, then capture the outgoing user's CPU state and install the
clean kernel image before handler policy. Restore only the selected owner's
image.
Kernel-origin interrupts must not reborrow an active userspace gate. Restore
bootstrap root, TSS, CPU controls and continuation before releasing any owner.

Install four explicit directed channels: workspace to editor, editor to
workspace, editor to service, and service to editor. Bind send/receive authority
to task and endpoint incarnations. A public role label or packet sequence is not
authority. Retirement closes only incident routes; service retirement must leave
workspace/editor communication and scheduling available. All resource retirement
must complete through the single coordinator.

Gate selection is explicit. The workspace-only `f3` path is a bounded probe at
this checkpoint; it does not admit arbitrary existing mailbox operations.
The `f4` path carries fixture calls and copied packets under role checks. No
physical input, framebuffer or disk grant is installed by this admission proof.
Unauthorized requests must return attributed rejection without consuming or
publishing packet state.

### Implemented probe contract

Three independent leaf assembly payloads enter CPL3 under the actual role roots.
Each proves initial clean CPU state, installs its own x87 value and XMM0 pattern,
and remains selected for at least two checked user timer interrupts. Cooperative
yield and terminal dispatch preserve those values. Only the timer IRQ is enabled;
this does not qualify physical input or nested kernel idle interrupts.

The four channels carry six known eight-byte packets. Wrong-role send/receive,
editor/service `f3`, and workspace/editor service-probe calls are denied. Invalid
copy spans at the workspace arena guards and at editor/workspace receive
boundaries leave queued packets and destination sentinels unchanged. The
service-only probe is a role check, not a storage-device operation.

An exact service UD2 marker retires the service only after all first-round
packets are acknowledged. Its two empty incident routes close; the workspace
and editor then exchange two further packets before normal exit. There is no
pending-packet discard, unknown-save, replacement or restart claim in this gate.
Final cleanup must revoke the old task identities and return allocator, context,
circle, scheduler and CPU ownership to the recorded baseline.

The workspace arena is read back before and after service retirement. The final
check must preserve its original pattern, allowing only the sent `WOR2` word at
offset 64 and received `EDI2` word at offset 256. Do not refill the arena before
this comparison: the service arena overlaps its virtual addresses, so retained
bytes are part of the cross-root isolation evidence.

## Qualification

Test exact geometry, incompatible legacy constructors, guard/PTE failures,
zeroing, checked-copy boundaries and retained failure cleanup. Inspect the emitted
fixture's instructions and stack use, selected runtime and kernel stack/control
paths. Run real three-task entry, distinct CPU state, all four copied routes,
wrong-role rejection, service retirement with surviving workspace/editor progress,
and exact final ownership restoration under TCG and available KVM at 96 and
320 MiB. Attempt VirtualBox without weakening CPU capability admission.

Run all sixteen repository-required checks and affected native/workspace
regressions. Record final image hashes and compare existing profiles with their
starting artifacts. Serialize builds and VMs under the established memory limits
and system/ancestor PSI guard; retain failed attempts and journal evidence.

The final candidate `a425ebd` completed all four 96/320 MiB TCG/KVM boots with
full arena retention and exact final resource/CPU restoration. The predecessor's
refill did not establish retention across service activity and remains separate
evidence. Final emitted bounds, artifact bindings, all sixteen required checks
and affected regressions are recorded in the [mixed admission log](../../notebook/Development/2026-10-05%20Mixed%20Native%20Workspace%20Log.md).

VirtualBox 7.2.6 reached the kernel but refused this CPU profile before userspace
entry with `application_cpu_amd_error_pointers`: its AMD guest CPU does not expose
the required XSaveErPtr guarantee. The isolated VM was powered off. This is a
retained capability refusal, not successful VirtualBox qualification; the CPU
admission policy remains unchanged.

## Remaining integration

The remote editor adapter must derive copied action/result authority from the
installed routes while retaining the workspace's publication fence locally.
It must render authenticated ANUI observations into these admitted frame slots
without constructing a shadow editor owner. The real workspace, sole editor
session and durable service then need full emitted-stack review, physical input,
composed pixels, delayed-save and service-loss behavior, cold recovery and cleanup.
This gate does not enable a launcher entry, define a stable executable format,
or establish an Electron/Antimatter port.

[ADR 0071](0071-compose-a-copied-remote-editor-adapter.md) now implements the
opt-in copied application codec and workspace receipt adapter on the host,
without a shadow editor. Binding its trusted receive seam to the actual native
routes and selecting the three real payloads remain separate guest work.

The subsequent [ADR 0072](0072-integrate-a-native-editor-workspace.md) implements
that native integration with the real workspace, sole editor session and durable
service. Its sixteen-boot TCG/KVM matrix and final emitted CPU/stack review now
cover physical input, composed pixels, delayed-save/service-loss behavior, cold
recovery and cleanup. Its required checks, regressions and final audit also
passed. This follow-on does not expand the claims
of the admission probes above or establish a complete Electron Antimatter port.
