---
title: "Interactive Durable Editor Log"
date: 2026-10-05
tags: [native, editor, input, storage, qualification]
status: qualified
---

# Interactive Durable Editor Log

## Scope

[ADR 0065](../../docs/adr/0065-prove-an-interactive-native-durable-editor.md)
connects physical keyboard editing and live presentation to a separately owned
native durable service. The editor owns one non-copyable session; the service
alone holds the device grant. Four opaque routes retain the existing native
copy/acknowledgement discipline. Complete staged presentation remains available
after service retirement, so an uncertain save can stay visible and editable.

This is an isolated Rust reference pair, not the Electron/React/CodeMirror
Antimatter frontend or the workspace launcher integration. F4 Editor Lab stays
public and memory-only. The new gate uses a public disposable fixture, a 31-byte
ASCII line and one-step undo. Those are local proof bounds, not native path/name
limits or a general filesystem implementation. ELF comparison of this live
pair remains a following gate; existing native/ELF replacement proofs remain
separate regression requirements.

The next integration is an owner-bound workspace surface/input/observation
bridge for this separately owned editor. It must preserve one save owner and
keep document content out of unauthorized public desktop snapshots.

## Implementation and source review

Physical input and complete-row presentation belong only to the editor. Storage
read/write/flush belong only to the service. Copied startup handles cannot
create authority. ANDI startup and the ANDF boot fixture contain phase/mode and
installed bindings, with no initial document bytes or expected recovery answer.
A 128-byte ANUI audit describes the published session through its public route.
After service exit that route closes: subsequent hidden metadata is not attested
by an audit, but complete independently predicted frame hashes and pixels still
show the uncertain outcome and newer edits.

The session retains pending bytes, request identity and revision while editing.
A late exact reply changes the saved baseline without discarding newer text.
An uncertain outcome never permits a second save or a false clean indication.
Dirty/pending/unknown close requires two consecutive confirmations; clean close
uses the authenticated document close exchange. Pending presentation and its
audit ACK precede request submission. Service proof modes hold the first actual
saved reply until newer editing, or commit and retire without submitting it.

Review caught a live framebuffer mapping issue before any VM ran. The fix
captures the exact user's CPU state, checks its retained owner/root/TSS and
actual supervisor stack, temporarily selects the bootstrap CR3 for integer-only
presentation, then restores and validates the exact owner root. The callback
cannot dispatch, enable interrupts or copy user memory; invariant errors are
fail-stop. It adds no user framebuffer mapping or retained page budget. Idle
STI/HLT/CLI retains its memory clobber and checks for pending physical input with
interrupts disabled. Kernel-origin idle IRQs do not reborrow the active runtime.

Initial failed attempts are retained: one oversized renderer test fixture,
missing feature-selection entries, getter/dispatch type compile errors and a
linked inspector still expecting the intentionally unexecuted scripted baseline.
All were corrected rather than weakening successful-run evidence. The baseline
payload is now required to be collected from the interactive linked image.

## Exact emitted artifacts

Evidence is under
an external validation directory (not distributed).
`artifacts/`, `client-review/` and `kernel-review/` retain exact binaries,
disassembly, control/stack analyses and hash manifests.

| Artifact | SHA-256 |
| --- | --- |
| Client ELF | `aad5f1ce827b232af690140771abcbb23faad50933ba640700d75327afa42b3a` |
| Client RX | `70f7502861b43160a3d32dfda35fb478e75f110f9a2567597c823cb877a65023` |
| Kernel ELF | `8145f1072bb4deda2163632ddd50a47e19c829d79a75dbe9e3a3ae3f4deefb0a` |
| Boot image | `f3c90d19b75365f8b3d0bee8d84ad193e383958e8fe471cf9678fe6eea864d95` |

Client RX is 26,348/32,768 bytes; its initialized RW is 4,288 zero bytes and
there is no BSS. Arena allocations use 21,248/32,768 bytes. The conservative
client stack bound is 4,928/8,192 bytes including panic paths. All 68 local CFGs
and eight jump tables close without recursion, unresolved transfers, dynamic
stack adjustment, red-zone use or compiler SIMD.

Kernel review closes 197 functions, 3,647 direct transfers, 48 manually resolved
indirect sites and nine tables. The conservative bootstrap bound including IRQ
is 25,687/65,536 bytes; native gate including nested idle IRQ is 9,662/16,384.
The file end is `0x12a000` and resident end `0x1aa000`, below unchanged limits
`0x160000` and `0x200000`. No compiler SIMD or unresolved admitted transfer
remains. The 26,348-byte client payload matches the reviewed ELF; the 172,032-byte
kernel load occurs exactly once in the inspected boot image. These are emitted
conservative bounds, not dynamic high-water measurements.

## Physical editing, delayed replies and cold recovery

`make check` passed in 557.63 seconds. The final host-only harness changes bound
reads before allocation, allow terminal monitor EOF only provisionally, and use
repository identifier style. All eight mutation tests were rerun afterward;
the native artifacts remained byte-identical. The final source manifest covers
412 non-Markdown files. The guarded first 96 MiB case passed, followed by the
full eight-boot matrix at 96 and 320 MiB on the exact retained image.

The full trace checks 62 complete frame hashes, 48 stable full-screen captures,
112 physical input deliveries and 8,928 accepted rows. Eight boots retire 16
owners with resources restored and authority revoked, no input loss, no faults
and no discarded packets. Every boot reports six role denials, one bad copy,
three invalid operations and one closed-route observation. Typed host entropy
is delivered after boot through PS/2, never injected through startup. Captures
also verify pixels outside the granted rectangle remain unchanged.

Mode 1 saves B while editing C, accepts the older reply with C still dirty,
then saves C and closes cleanly. Cold recovery reports C at content version 3
with a fresh mount epoch 2. Mode 2 commits B and retires without a saved reply;
the editor retains unknown B while displaying newer C and requiring explicit
discard confirmation. Cold recovery reports B at version 2 and epoch 2. No
recovery guest receives the predicted text. A host parser independently checks
both complete alternating checkpoints, CRC32C, predecessor content, exact
identity/counters, reserved bytes and every byte outside the five-sector extent.
Neither result claims a persistent request-result ledger or arbitrary power-cut
qualification.

The transcript oracle rejects missing/extra/reordered records, incorrect field
sets/types, packet/frame changes, incorrect ACK/cleanup counts and truncated
JSON. Literal host ANUI/ANDC expectations do not replay the guest editor model.
The pixel reference shares the renderer; independent glyph/caret unit assertions
supplement it. Fixed HMP release timing can cause a failed run if the VM is too
slow; it cannot admit an incorrect input count. Input-loss handling and hostile
service-fault recovery are not newly exercised by this physical trace.

## Regression and resource qualification

All 16 required targets passed: `check`, `smoke`, `fault-smoke`, `circle-smoke`,
`control-smoke`, `rite-smoke`, `scheduler-smoke`, `supervision-smoke`,
`shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`, `input-wait-smoke`,
`shell-smoke`, `semantic-surface-transport-smoke`,
`semantic-presentation-smoke` and `normal-exclusion`.

The five legacy repeated/durable/failure and native/ELF replacement smoke
commands passed. Their exact artifacts are retained in `regression-artifacts/`.
Independent delta review closes equivalent unsigned bounds predicates, function
relocations and their references, embedded payload updates and decoded panic
source-line records. Kernel executable sections and allocated geometry remain
identical to the preceding qualification. Existing client bounds carry forward:
repeated 6,000, durable 5,408, failure 5,904 and replacement 3,904 bytes of 8,192.
Whole-file identity is not claimed. The native/ELF replacement clients remain
exact twins at `a39228292fe0cf79c5e0d8e27f61eadee959570ba8cb19061f92d07fc93aee27`;
ELF loader code/plan are unchanged. The comparison report is
`regression-review/baseline-review.json`, SHA-256
`7516229131bc91373e1df390ad9b0e427cd63edad38430b4f8c0c5820d20a308`.

The workspace editor passed its 95-frame trace. Existing desktop regressions
passed 21 keyboard, 465 pointer/Appearance and 26 Graphics frames, including
fault/watchdog, withdrawal, fresh sessions and shell recovery. Isolated
VirtualBox matched all 786,432 initial-frame pixels and the boot lifecycle,
with explicit platform memory-map variation. The private VM and VBox service
remained in the memory scope and were shut down; no existing VM was adopted.
This qualifies the workspace initial frame in VirtualBox, not this new native
application profile. The existing native CPU admission limitation remains.

The final workspace artifacts are retained in `workspace-artifacts/`:

| Artifact | SHA-256 |
| --- | --- |
| Workspace client ELF | `93db24454d18740c4721bbcb2a19ecdf8e4d38df37bc49ee146cb28bfeb0cac0` |
| Workspace client RX | `5bce03f1562800c7fe01fc240a9cc3261a7f344146a38b41fbb5b1f0359fdbec` |
| Workspace kernel ELF | `f675b15661245e3a9cc17ae7415cad01c2ec31214fdb0ba4f4e93ff92855f07d` |
| Workspace boot image | `f96c18f51f45205b99135d5886e6375ec95006f07d2fb5018a0723e2bf92e046` |

The client comparison explains every changed loaded byte through two identical
functions exchanging positions, three updated function references and nineteen
rebased ASCII jump-table targets. Fresh control-flow and stack analysis closes
all 99 functions and 25 tables, preserving the 61,240/65,536-byte bound with
4,296 bytes spare. The same documented impossible UTF-8 panic recursion remains
excluded. Kernel executable bytes and allocated geometry are unchanged;
read-only differences are exactly the reviewed client payload and three decoded
panic line fields. Fresh kernel closure preserves the bootstrap-plus-IRQ bound
of 25,471/65,536 bytes and gate-plus-idle-IRQ bound of 5,326/16,384 bytes.
`workspace-review/` and `workspace-kernel-review/` retain the exact comparisons
and manifests. Payloads bind to the retained kernel and image used by the live
workspace tests and the isolated VirtualBox comparison.

Builds/tests/VMs remain serialized with two Cargo workers, one test thread,
4 GiB soft and 6 GiB hard memory limits, and a 512 MiB scope swap cap. The guard
samples host availability and both scope and host OOM counters. The complete
repository check peaked at 4,296,994,816 bytes, triggering soft-limit reclaim;
no hard-limit or OOM event occurred. Host availability stayed above
16,939,122,688 bytes across all retained guarded attempts. Every guarded attempt
has zero scope/host OOM events and zero hard-limit events. `qualification-summary.json`
records the complete passing matrix and source/release bindings. The unchanged
412-file source manifest hashes to
`cec4e21eae29fd8b6e88daad7254dc502a1310a8928d3e61f758186031fc864b`.
The four boots at each memory size report identical initial free frames, with
24,117 at 96 MiB and 81,461 at 320 MiB: exactly the expected 224 MiB difference.
The final ordinary image hashes to
`2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1`.

## Checkpoints

- `fefae17`: interactive session, observations and proposed ADR; 43 editor tests
  and warnings-denied Clippy passed before push.
- `3c08c61`: native runtime/client/host proof; complete repository check, exact
  emitted reviews and eight-boot interactive matrix passed before push.
