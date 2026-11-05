---
title: "Keyboard Editor Workspace Log"
date: 2026-10-05
tags: [native, editor, input, workspace, qualification]
status: qualified
---

# Keyboard Editor Workspace Log

## Scope

[ADR 0064](../../docs/adr/0064-add-a-public-keyboard-editor-workspace.md) adds
Editor Lab, a public memory-only scratch editor in the existing userspace
workspace. F4 and the sixth Applications entry open it. This is the interactive
input/rendering prerequisite to native Antimatter; it is not the actual
Electron/React/CodeMirror application, an independently isolated editor, or a
connection to the durable document service. Ctrl+S explicitly reports Save
unavailable. Closing discards scratch contents.

The copyable local model owns no service identity or pending save. It stages
repeated insertion/deletion, retains one immediate predecessor, and preserves
original caret/selection through undo. Canonical feedback records replacement,
rejection, no-op, undo, unsupported shortcuts and input resets. Editor commands
require observe/present authority, current revision and exact focused visible
window identity. Public text and metadata use the existing candidate/display/
acknowledgement/publication transaction.

Physical focus consumes printable text before desktop shortcuts. F1/modal
launcher and Tab focus remain global; Escape collapses selection in the editor.
Both modifier sides, Caps state, release and repeated-make suppression retain
exact source sequencing. Small geometry blocks physical edits using the same
visibility predicate as painting, with explicit enlargement feedback. Selection
has a contrasting underline even for spaces. The compositor paints rows directly
without another Graphics-sized pixel buffer.

## Review and preliminary checks

Independent source review found no authority/publication blocker. It identified
invisible physical editing at small sizes and low-contrast whitespace selection;
both were corrected before native execution. Host regressions cover retained
held keys across focus changes, fresh typing after source loss, and preservation
of undo when the editor is enlarged again. Initial compile attempts also found
a missing taskbar match and two test-harness binding/error-formatting issues.
The sixth launcher entry required moving a literal popup-header test point.
These failed attempts remain retained rather than being reported as passes.

The focused run passed 62 compositor, 59 workspace, 33 editor-contract, 57
workspace-service and 259 xtask tests, plus doc tests. The subsequently added
small-window input regression is included in the full repository check. Scoped
Clippy with warnings denied and the native workspace build passed.

The native smoke uses literal expected editor checkpoints, without calling the
keyboard/pointer adapter to predict actions. Those semantic assertions run on
the host model; guest observations are exact frame sequences and full RGB
comparisons. Hidden guest revision/dirty/history values are not independently
attested. Independent glyph/caret/selection unit tests complement the shared
host compositor comparison. Existing workspace pointer and Graphics smoke
traces remain required regression checks.

## Evidence location and resource policy

Local evidence is retained under
an external validation directory (not distributed).
Builds/tests/VMs are serialized, with two Cargo workers, one test thread,
4 GiB soft/6 GiB hard memory limits and a 512 MiB scope swap cap. The guard
samples host availability and scope/host OOM counters and aborts on pressure.
Source/configuration files are frozen independently of evolving Markdown notes.

## Native editor checkpoint

`make check` passed in 351.58 seconds. The existing Graphics oracle was then
updated to navigate Welcome → Editor → Graphics with two Up events. Targeted
workspace xtask tests and warnings-denied Clippy passed for that host-only
change; the final 403-file freeze is `source-final.json`. The native user/kernel
artifacts are unchanged by that oracle adjustment.

`make workspace-editor-smoke` passed 95 exact frames in a private QEMU process,
including repeated editing, selection/deletion/undo, errors, focused shortcut
isolation, modal input, release-activated close and fresh window incarnation,
launcher activation, unavailable Save and resource-restoring shell return.
Both retained captures were pixel-checked; the capacity-error capture was also
visually inspected. The whole reviewed boot image was copied into that VM.

The existing workspace smoke also passed: 21 keyboard frames, 465 pointer/
Appearance frames and 26 Graphics frames, retaining fault/watchdog/context/
mailbox withdrawal, fresh sessions and shell recovery. Pointer motion grows by
three frames with the taller menu; Graphics adds one explicit navigation frame.

The exact emitted workspace ELF `ab7ea5360014b047f8591b832c9aefb6ce9b883e0e20be916b479b4c318e7355`
has a conservative entry bound of 61,240/65,536 bytes, leaving 4,296 bytes.
Fresh closure covers 496 register transfers, 25 local jump tables, 1,804 direct
transfers and 99 balanced local CFGs. No unresolved admitted transfer, dynamic
stack adjustment, red-zone use or SIMD/x87 instruction remains. Two recursive
invalid-UTF8 diagnostic calls are excluded only under the documented fixed
ASCII paragraph-label invariant. This is an emitted conservative bound, not a
hardware high-water measurement. RX is 88,578 bytes; RW initialized data is 16
bytes, within unchanged admission limits. Exact payloads match the linked kernel.

The fresh workspace kernel review covers 138 admitted functions, 26 manually
closed indirect sites, six local tables and nine formatter alternatives. The
bootstrap bound is 25,471/65,536 bytes including one IRQ; the rite transition
bound is 5,326/16,384 including the nested IRQ admitted by idle yield. IRQ and
fault entries are 1,095 and 1,567 bytes. Nonlocal entry/resume, task identity,
mailbox admission, complete-frame publication and resource-restoring shell
continuation were reviewed on the exact emitted artifact. Source-supported
fatal Rust panic/string-slice diagnostics remain outside the certificate; it is
not a corrupted-memory proof. The complete loaded kernel bytes were matched to
the retained boot image. Full reports and hash manifests are in `stack-audit/`
and `kernel-review/`.

Implementation checkpoint `8f208df` was committed and pushed after host checks,
emitted review, native editor and existing workspace smoke passed. The remaining
matrix is a separate qualification checkpoint.

## Completed qualification

All 16 required repository commands passed. The serialized matrix also passed
native repeated editing, normal durable recovery, durable failure cuts, native
replacement and ELF replacement: 20 jobs after the separate `make check`.
The new editor and existing workspace proofs passed separately, followed by
five release jobs restoring normal/workspace images, binding the rebuilt
artifacts, booting isolated VirtualBox and verifying its result.

All five retained regression kernels and boot images are byte-identical to the
previous qualified checkpoint. Client ELF differences are confined to nonloaded
debug sections: loaded segments, executable/allocated sections, symbol tables
and unwind data are unchanged. The ELF loader and deployment ELF are wholly
identical. The freshly built normal image also retains its prior hash. These
comparisons preserve previous emitted certificates without claiming whole client
ELF identity. `regression-review/` retains exact comparison reports.

VirtualBox matched all 786,432 initial-frame RGB pixels and the expected boot
lifecycle, with explicit platform memory-map variation. Its private VM and
private VBox service remained inside the memory scope and the temporary VM was
powered off after capture. This qualifies initial workspace rendering; physical
editor interactions were exercised in QEMU. The existing user VM was not adopted
or modified. Normal and workspace development images are restored.

Final workspace image:
`68e2ff5bb5faccd75bff6e646dac127f998d84139f0f58f54036b403bf9eef1d`.
Final workspace kernel:
`67f6ec4ab1d32880ca49eb3dddf427652a0ca6cc37cc88662a05c10c8d8307b8`.
The rebuilt image/kernel/user payloads match the exact emitted reviews and the
private editor boot copy. The final 403-file source/configuration manifest is
unchanged, SHA-256
`46e0ef1df1115b5b9e634c25660707ee116636bc5bc14b47b1cfec3f9acfae62`.
The only change from the initial check freeze was the Graphics host-oracle
navigation adjustment, followed by targeted tests, Clippy and native frame proof.

Across all 11 memory scopes, including early failed host-test attempts, peak
scope usage was 2,133,942,272 bytes (1.99 GiB) and minimum host availability was
18,903,080,960 bytes (17.60 GiB). No soft/hard-limit event, scope OOM or host OOM
increase occurred. `qualification-summary.json` binds results, counts, artifact
identity, source freeze and memory summaries; failed attempts remain separately
recorded. No generated build/target files are committed.

## Next integration gate

Route focused editing to a separately owned native application and connect its
existing authenticated durable document service. Keep exact pending-save
identity during typing, and expose save/rejection/unknown outcomes visibly and
semantically. Then compare the same behavior through ELF. The actual Antimatter
frontend/runtime port remains necessary for product Milestone 1.
