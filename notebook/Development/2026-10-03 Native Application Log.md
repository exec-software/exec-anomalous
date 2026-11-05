---
title: "Native Application Surface and Input Contract"
date: 2026-10-03
status: bounded-qualification-complete
---

# Native application surface and input contract

This increment implements the bounded proof in proposed
[ADR 0050](../../docs/adr/0050-prove-a-bounded-native-application-contract.md).
All 33 final matrix jobs and both restoration jobs passed on 291 unchanged
frozen non-documentation files. Native TCG and actual-host KVM runs passed at
both memory sizes and in separate capture runs. ADR 0050 remains Proposed;
this qualifies one bounded contract prototype, not a stable application ABI.
The preceding [Ny object checkpoint](2026-10-03%20Ny%20Object%20Log.md) retains
its own separate evidence and limits.

The user selected the native application ABI baseline first, a separately
qualified ELF path next, then comparison with the Linux Antimatter copy on the
host outside the VM. Windows and macOS comparisons follow later. Antimatter
remains the official editor and Ny the official language direction. This first
service proof uses Rust to reuse verified mechanisms and carry a bounded part
of actual Antimatter editor behavior into independent userspace.

## Real editor behavior and the host reference

The unsafe-free, allocation-free
[`anomalous-editor-contract`](../../crates/anomalous-editor-contract/src/lib.rs)
ports selection replacement from Antimatter's production
`renderer/src/environment/editorBridge.ts::insertAtCursor`. It follows the
document, selection, history and appearance behavior covered by
`test/editor-terminal-theme.test.ts`. The source audit used Antimatter commit
`0679c44`; the reference runner records hashes of the actual imported files.
Only printable ASCII is admitted, where byte and CodeMirror UTF-16 offsets
agree. The model holds at most 31 bytes and one undo entry.

Run the existing checkout's actual CodeMirror modules and production bridge with:

```sh
node scripts/native-application-reference.cjs /path/to/exec-antimatter
```

The [runner](../../scripts/native-application-reference.cjs) installs nothing,
bundles in memory with that checkout's installed dependencies, and compares the
result with the checked
[reference trace](../../tools/xtask/testdata/antimatter-editor-reference.json).
It exercises the editor state and production insertion bridge, not an Electron
window or an Anomalous Linux personality. The native service and host oracle
compare the same seven states:

| Frame | Operation just applied | Text | Selection | Revision / dirty / undo |
| ---: | --- | --- | --- | --- |
| 1 | Initial dark state | `const before = 1;` | 0, 0 | 1 / false / 0 |
| 2 | Select replacement range | `const before = 1;` | 6, 12 | 1 / false / 0 |
| 3 | Production-style insertion of `after` | `const after = 1;` | 11, 11 | 2 / true / 1 |
| 4 | Light theme | `const after = 1;` | 11, 11 | 2 / true / 1 |
| 5 | Dark theme | `const after = 1;` | 11, 11 | 2 / true / 1 |
| 6 | Move caret | `const after = 1;` | 8, 8 | 2 / true / 1 |
| 7 | Undo | `const before = 1;` | 6, 12 | 3 / false / 0 |

The CPU renderer draws the actual text, selection and caret into 128×8 RGB8.
It uses Antimatter's base light/dark colors and Anomalous's existing MIT 5×7
bitmap font. Browser typography, syntax highlighting, Unicode shaping and
full CodeMirror history are outside this model.

## Independent client, service and retained ownership

The same inspected
[native client image](../../vessels/anomalous-native-client/src/main.rs) runs
as two separate tasks: editor producer and scripted surface/input service.
Each has eight RX pages, two private data pages, a guarded two-page user stack
and a retained privilege stack. Large buffers live in private BSS; boot
arguments occupy a separately reserved 128-byte prefix. The host ELF is only
the build container: tooling inspects and extracts fixed RX bytes, while the
kernel supplies zeroed data and internal launch bounds. There is no ELF loader
or ELF execution ABI in this path.

Canonical [wire packets](../../crates/anomalous-abi/src/application.rs) carry
3,168-byte surfaces and 64-byte semantic input. Two opposite-direction
[channels](../../kernel/src/application_channel.rs) bind exact task generations
and own copied packets through acknowledgement. They carry no caller-asserted
authority. Kernel transport treats editor content as opaque; model and rendering
policy stay in userspace. Each copy remains within the 4,096-byte checked-copy
limit and preflights the complete current-task range before mutation.

After each successful surface send, the editor physically overwrites its
source packet with volatile `0xa5` stores before yielding. The service must
receive the earlier snapshot. Wrong-role operations, a failed source copy,
failed receive copy and insufficient receive capacity must preserve the valid
retry. The service acknowledges the seventh surface and yields so the editor
can observe acknowledgement and exit. It retains its independent final copy,
scales it three times, and stages 24 copied rows for the 384×24 rectangle at
`(32, 32)`. Only after both tasks retire and bootstrap resumes does the kernel
commit/read back the full rectangle and verify unchanged pixels outside it.

The second pair queues a first surface, then the producer reaches one exact
armed UD2. Both channels close before the peer resumes. The service must see
closed-channel rejection, an unchanged receive destination, no delivery and
no new display rows. The previously presented frame remains unchanged.
Both scenarios retain mappings, CPU images and stacks through terminal context
withdrawal and cleanup. Cooperative yields switch tasks; PIT interrupts resume
the same current task. Timer counts do not imply timer-driven scheduling here.

## Development validation and corrections

Implementation and source review preceded the final qualification below.
The first client compilation found a Rust
static/parameter name collision; the private static was renamed without changing
the protocol. Building `core` and `compiler_builtins` with memory intrinsics and
the selected direct-libcall configuration eliminated unsupported GOT dependencies;
artifact checks still forbid GOT/PLT sections. Explicit writable assembly storage
fixed the boot-argument section's flags, and declared NOP gap fill replaced the
linker's default trap padding. The first native attempt exposed the earlier
prepared-launch owner's one-page code assumption; an explicit bounded one/eight-
page profile and corresponding host tests now preserve that owner's geometry
checks. Failed attempts remain in the external evidence directory.

The focused native-feature kernel run passed 460 tests, and client/kernel
warning-denied Clippy passed. Preliminary TCG runs passed at 96 MiB and 320 MiB
with exact normal/fault call counts 112/12, cooperative switches 14/0, seven
surfaces, six inputs, three normal exits and one contained fault across four
tasks. Row staging, framebuffer readback and preservation of the earlier frame
all passed. Visual inspection of the captured crop confirmed the actual
`const before = 1;` text with restored selection 6 through 12 and caret.
These development observations do not replace the final frozen-source matrix.

Evidence is retained at
an external validation directory (not distributed), including
`native-application-frame.png`, `native-application-editor-crop.png` and
`tcg-artifact/qualification.txt`. The latter records the inspected client:

| Item | Value |
| --- | --- |
| Initialized RX extent | 8,496 bytes, within the allocated eight pages |
| RW extent including boot arguments and BSS | 7,584 bytes, within two pages |
| Client ELF SHA-256 | `af2b3d6c68d036f42a80e2e57f5a8596944383e4636bd287cac10371e671bd7d` |
| Extracted RX SHA-256 | `000374ca2704b22cfa7ce0ba134cbbe5d13617f0438ec0a15fba35c1964488c0` |

Independent read-only stack inspection of this emitted client found a largest
individual frame of 424 bytes for the producer, 376 for the service, and 216
for rendering/receive. Conservative direct-call accounting, including return
addresses, tail jumps counted as extra calls and panic paths, totals 888 bytes
from entry. There is no recursion, indirect call or dynamic RSP adjustment;
the four compiler match-table jumps stay within their containing functions.
This leaves margin under the guarded 8 KiB user stack, with interrupts using
separate privilege stacks. This is artifact-specific inspection, not a general
stack-analysis guarantee for future compiled clients.

The inspected kernel ELF hash was
`7716c358144a9e264ee196acc54b9e6424707fcf3934ee43821328e967c71ed6`, before the
prepared-launch geometry correction. Its inlined packet/gate dispatcher frame
uses 4,440 bytes; the largest copy preflight frame uses 536 bytes. Known direct
call accounting reaches 5,856 bytes including entry to fatal formatting, plus
the 176-byte normalized interrupt frame and at most 15 bytes of alignment.
Resolved memory-libcall wrappers use eight-byte frames. Fatal formatting has
indirect callbacks, so this is a stack-risk review of the 16 KiB dispatch/copy
path, not a complete bound for all diagnostics or a final-kernel hash claim.

## Final frozen-source qualification

All 33 matrix jobs passed in 1,427.39 seconds of summed job time, followed by
two successful image-restoration jobs. The fresh `make check` took 321.58 seconds
and included 494 all-feature kernel tests, 460 native-feature kernel tests and
132 xtask tests. Model/codec rejection, channel/scheduler atomicity, artifact
mutation, transcript/screenshot, regression and normal-exclusion checks passed.
The durable host reference command also passed against the actual Antimatter
checkout. All 291 frozen non-documentation files were unchanged afterward,
with no added source files.

TCG and actual-host KVM each passed separate 96-MiB and 320-MiB debug-exit boots
and a capture boot. Each complete native lifecycle validated 21 records. The
normal/fault scenarios retained exact 112/12 call counts and 14/0 cooperative
switches, seven delivered surfaces, six delivered inputs, three normal exits
and one contained fault across four tasks. Both backends used the same native
image and produced byte-identical public editor rectangles. The retained final
PNG and crop now come from the KVM capture. Timer observations remain separate
from cooperative scheduling and are not performance claims.
TCG used the Nehalem `GenuineIntel` baseline and recorded normal/fault timer
counts 2/1 at each RAM size; KVM used the actual `AuthenticAMD` host CPU on q35
and recorded 16/19 at each size.

The normal image was restored byte-identically to the previous checkpoint.
The workspace image changed with the shared framebuffer-presenter refactor;
both its TCG and KVM smoke tests passed. Do not report that workspace image as
unchanged.

| Final image | SHA-256 |
| --- | --- |
| Native application proof, TCG and KVM | `b2559c863c71dd5f77fc54461b106b481a6de0a6e4f990f8705121796912ba45` |
| Restored normal image | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| Restored workspace image | `d6c38325c57d33677aca118772f3cf69036293b73bed1bc1bec1e3c275ecd546` |

The matrix ran with two Cargo jobs, a 4 GiB soft limit, 6 GiB hard limit and
512 MiB scope swap cap. Peak scope memory was 4,297,056,256 bytes (4.002 GiB);
host available memory stayed at or above 19,943,706,624 bytes (18.57 GiB).
There were 2,055 soft-limit reclaim events, zero hard-limit events, zero scope
OOM events or kills, and zero host OOM kills before or after the run. The watcher
did not abort. The restoration scope peaked at approximately 31 MB with no
memory events.

The evidence directory above retains `final-receipt.json`,
`matrix-final-results.json`, `matrix-final-memory-summary.json`,
`restore-final-results.json`, `source-freeze.json` and `reference-final.log`;
`evidence.sha256` covers 170 retained evidence files.
The client artifact hashes and stack inspection recorded above still match the
qualified client. Reproduce artifact and native checks with
`make native-application-inspect`, `make native-application-image` and
`make native-application-smoke` under the same memory controls.

## Limits and next gate

This is an experimental Rust native-contract prototype with scripted semantic
input and one deferred final presentation. It is not the full Electron editor,
a Ny frontend/runtime port, physical keyboard routing, an interactive compositor,
a stable application ABI, a file service or an ELF execution path. The later
ELF adapter must name its supported execution/service personality and compare
common behavior against this baseline. Workspace open/edit/save/relaunch,
broader editor parity, HAG acceleration and Windows/macOS execution need
separate implementation and evidence.

The ELF follow-up has two distinct gates. A bounded program-header adapter can
first feed inert file ranges, zero-fill, entry and permission requests into the
retained native launch path, then replay this same golden trace. That qualifies
a container/loading route only. A native-versus-Linux execution-ABI comparison
also needs a separately specified Linux startup and syscall personality mapped
to the same ownership and application services. The current host-only ELF
inspector already flattens the native client; relabeling that step would prove
neither a guest loader nor another execution ABI. Constrained guest parsing,
initialized-data population and BSS clearing need their own negative evidence;
artifact metadata must never supply channel grants.
