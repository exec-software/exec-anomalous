---
title: "Editor Workspace Bridge Log"
date: 2026-10-05
tags: [native, editor, workspace, ownership, qualification]
status: portable-qualified-native-integration-pending
---

# Editor Workspace Bridge Log

## Scope and native admission finding

[ADR 0066](../../docs/adr/0066-model-an-owner-bound-editor-workspace-bridge.md)
adds a portable, opt-in bridge between the real interactive session, workspace
transaction model and compositor. This is a host-qualified prerequisite for
native integration. It adds no VM application, native route, launcher entry,
Electron frontend, filesystem namespace or private-document disclosure policy.

The native workspace currently uses a different execution path from the editor
and durable service. Its 88,578-byte RX payload and 61,240/65,536-byte stack bound
cannot fit the application's 32 KiB code/8 KiB stack admission. Existing owned
root planning also assumes a low branch absent from the old high-VA workspace
layout. Increasing the task count alone would not establish safe integration.

The next native gate should explicitly admit a separately linked workspace
profile with a 64 KiB stack, larger code/data spans and qualified observation
storage. Then one coordinator can retain workspace, editor and storage owners,
with the workspace alone receiving physical input and publishing complete
frames. Service failure must leave editor/workspace routes alive. Cross-task
input requires a focus fence for already-delivered actions. The current pair's
blanket route closure and `1 - selected` peer selection cannot be copied into
that coordinator. Estimated future frame counts are not qualified budgets.

## Implemented portable boundary

The default-off `native_editor_bridge` feature provides a singleton Native
Editor window. Its desktop snapshot has surface metadata and ordinary geometry,
never document bytes or a save owner. Graphics Lab and public scratch Editor
Lab retain their existing APIs and behavior. Generic create/close cannot bypass
the native window's scoped opening and application close policy.

`native_editor_owner` retains the non-copyable interactive session. The separate
`native_editor_bridge` retains one immutable ANUI observation and owned RGB frame.
Each frame is 18,432 bytes; committed and pending storage needs 36,864 bytes,
plus metadata and call temporaries. This portable implementation deliberately
claims no native stack bound or admission for those large owned values. Its
rejections return complete retained ownership without allocating a Box; future
native placement needs explicit arena/layout work, not a larger hidden stack.

Trusted setup selects `public_fixture` disclosure and binds desktop identity,
producer/client generation, exact window incarnation and nonwrapping attachment
generation. An observer needs the live desktop and a current observe-granted
endpoint to resolve the sidecar. Public descriptors and ANUI bytes are neither
capabilities nor native task authentication. Human and machine endpoints use
the same observation/action policy.

One move-only update retains the desktop ticket and next observation/pixels.
Publication preflights all fallible checks, publishes the desktop and then
installs the retained sidecar without another fallible state transition.
Cancel-before-begin returns the pending frame for retry. Rejected publication
or cancellation returns the entire update. A save acknowledgement or local edit
already accepted by the actual owner never rolls back with desktop cancellation.
Presentation attestation remains an explicit trusted local assertion.

One in-flight action uses a move-only permit, separate action sequence and exact
presented session sequence. Consumption rechecks current desktop revision,
focus, modal input, visibility, grants, attachment and actual editor sequence.
A save acknowledgement arriving after action preparation makes the old action
stale without effects; the caller must publish a fresh observation and resubmit.
The bridge translates no raw keyboard records. Native physical input policy and
cross-task synchronization remain subsequent adapter work.

Review caught a ticket-retention bug before the focused test run: foreign
consumption/cancellation discarded a ticket still needed by its rightful
bridge. Rejection now returns that unrecognized ticket intact. Exact locally
owned stale actions are consumed without effects and release their in-flight
slot. A two-bridge test verifies recovery after a foreign call. Dropping a live
ticket still does not implicitly cancel its retained transaction.

Normal close uses the editor's existing policy: clean idle state closes immediately;
dirty, pending or unknown state requires confirmation. Only an exact closed
observation permits window removal. A separate controller-only
terminal assertion permits ordinary transactional withdrawal without claiming
save success or application-confirmed discard. Old actions and observations
cannot target a replacement window. A surviving session cannot yet reattach to
a replacement desktop: retained-bridge retry and fresh-owner replacement are
the supported local recovery paths. Logical retirement claims no secure erasure.

## Focused evidence

Evidence is retained under
an external validation directory (not distributed).
The first compilation, library Clippy check and five-job focused run passed.

- Window surfaces: 16 unit tests and three compile-fail borrow tests.
- Opt-in workspace: 79 unit tests and 13 compile-fail ownership tests, including
  15 adversarial bridge tests and five native-window test groups.
- Opt-in compositor: 62 existing unit tests plus three new integration traces.
- Workspace/compositor warnings-denied Clippy with the feature enabled.

The integration traces use an actual `durable_store` over a host test device
with distinct volatile/persisted sectors. They exercise delayed B acknowledgement
while C is current, canceled/retried display publication, subsequent C save and
recovery from persisted media. A separate trace commits B and loses its reply,
undoes to A, preserves unknown/dirty state and exact pending B, disarms close on
input reset, then explicitly confirms close. This is host media-model recovery,
not another cold VM or physical-disk qualification.

Each asserted desktop publication composes all 768 rows with its exact retained
content. Literal semantic expectations check displayed sequence, revision,
version, dirty state, pending request/revision and outcome, with observer parity.
Geometry tests cover movement, resize, minimize/restore, focus, launcher modality
and occlusion; malformed/missing/stale binding sets reject before composition.
The renderer is shared with the native proof; matching full frames alone is not
an independent glyph implementation or physical scanout proof.

## Regression qualification

The first complete `make check` passed in 412.61 seconds. Independent source
review then identified a Cargo feature-unification issue: enabling only the
workspace extension through a sibling could add a window kind while leaving
the compositor's local feature disabled. Generic external-content fallbacks
now handle that combination, and the exact mixed-package feature check is part
of `make check`. The compositor tests/Clippy and mixed-feature check passed
after this correction. The final-source `make check` then passed in 549.15
seconds, including the new mixed-feature check and freestanding Clippy for both
opt-in libraries. All 15 required smoke/exclusion targets passed afterward, for
all 16 required checks on the final implementation. The 416-file non-Markdown
source manifest remained unchanged through the final run; its SHA-256 is
`63295a10e2d7f8455c162f6a9e25d86177b6e8eb5c50f8f0f60737a546f46b87`.

The serialized qualification run also passed a fresh durable-profile build,
default artifact comparison, both workspace regressions and an isolated
VirtualBox check. `workspace-editor-smoke` passed 95 exact physical-editing
frames. `workspace-smoke` passed 21 keyboard, 465 pointer/Appearance and 26
Graphics frames, including lifecycle/fault cleanup and working shell fallback.
These guest tests exercise the unchanged default workspace, not the new bridge.

The private 256 MiB VirtualBox VM emitted the expected 11-record initial boot
lifecycle, with explicitly allowed memory-map variation. Its 786,432 RGB pixels
matched the QEMU initial frame exactly; RGB SHA-256 is
`5c4bc135bded8d9be6839338cb3187622e7a0566929df25de943098c264e9041`.
Both VBoxHeadless and its private VBoxSVC remained inside the memory guard's
cgroup, and the VM was powered off cleanly. The existing user VM was not adopted.

### Default artifact and stack evidence

`default-artifact-comparison.json` covers exactly 23 retained/current files:
the five prior repeated/durable/failure/native-relaunch/ELF-relaunch profiles,
the interactive native profile, workspace and normal boot image. All listed
runtime images and flat payloads are byte-identical to the prior qualification;
ELF LOAD headers and bytes are identical. Its SHA-256 is
`605f00c55652805cd662075c00a67dc25a4a150146c3a6745bf6527b8ff3f749`.

The interactive client and workspace whole-file ELFs changed only in nonloaded
debug sections. Independent review additionally checked ELF entry/type/machine/
flags, every program header, allocated sections, symbols/string tables and
`.debug_frame`. All matched. This retains prior emitted stack evidence for the
same executable bytes and layouts: interactive client 4,928/8,192 bytes,
interactive kernel gate plus nested idle IRQ 9,662/16,384, workspace client
61,240/65,536, workspace gate plus idle IRQ 5,326/16,384 and bootstrap plus IRQ
25,471/65,536. The prior five profile and eight-boot interactive qualifications
are retained evidence, not claimed reruns in this checkpoint.

The independent verifier and hash-bound receipt are retained as
`verify_default_artifact_audit.py` and `independent-default-artifact-audit.json`.
The receipt SHA-256 is
`a13a853f11800511d5374946679ab1b9f4c41a5f59cbd24b5f123fb7cc97a2c4`.

Current retained workspace artifacts:

| Artifact | SHA-256 |
| --- | --- |
| Kernel ELF | `f675b15661245e3a9cc17ae7415cad01c2ec31214fdb0ba4f4e93ff92855f07d` |
| Workspace ELF | `c07402b1a72bbff3b14eb1bf3ef5c61f4f3568d7e58998a72bb46d7adc86b948` |
| Workspace RX | `5bce03f1562800c7fe01fc240a9cc3261a7f344146a38b41fbb5b1f0359fdbec` |
| Workspace data | `814dd7b9784d57c15b9c2972e9b4fd6cf7e164f8162a934bdb2452a413dab1f7` |
| Workspace boot image | `f96c18f51f45205b99135d5886e6375ec95006f07d2fb5018a0723e2bf92e046` |

No new native bridge execution or native bridge stack certificate is claimed.
The default-off artifact comparison is not a qualification of the larger native
workspace admission or three-task coordinator still required.

### Hosted preview and memory guard

The external `preview/` harness creates the real model, owner, window and bridge.
It composes three complete frames, binds an input grant, inserts one byte, then
publishes the resulting `Antimatter ABI` observation with caret 14, revision 2,
version 1 and dirty state. Human/machine observations match. `host-preview.json`
explicitly records host-only execution, no native transport and no document
service execution. The PPM was encoded to PNG and visually inspected; no pixels
were synthesized outside the compositor. The small reference surface is scaled
inside the workspace window, not an Electron frontend.

The first preview attempt caught a harness assertion expecting caret 20 for a
21-byte string. The external fixture was shortened to 14 bytes and its literal
expectation corrected; the guarded retry passed. This changed no repository
source. All 23 intended qualification jobs ultimately passed across 24 attempts;
the original failure and retry logs remain separate. Required checks did not
need repeating after this fixture-only correction.

Builds/tests/VMs ran serially with two build jobs, one test thread, MemoryHigh
4 GiB, MemoryMax 6 GiB and MemorySwapMax 512 MiB. Across all guarded runs the
highest scope peak was 2,226,868,224 bytes (2.074 GiB), lowest host availability
15,545,225,216 bytes (14.478 GiB), and largest sampled scope swap 22,433,792 bytes
(21.395 MiB). Scope high/max/OOM events and host OOM-kill deltas were zero; no
memory-pressure abort occurred. Host-wide swap is not attributed to this task.
`qualification-summary.json` retains the aggregate results, source manifest,
artifact digests, VirtualBox verification and hosted preview receipt.

## Pushed checkpoints

- `6b5e6e1`: owned immutable content frames; focused surface tests passed.
- `9ee8cdc`: portable bridge, independent composition/durable tests and ADR;
  the five-job focused test/lint run passed before push.
- `a8a3c0d`: feature-unification correction and explicit mixed-package check;
  compositor tests and Clippy passed before push.
