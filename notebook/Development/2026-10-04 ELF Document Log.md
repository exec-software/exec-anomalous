# ELF document comparison — 2026-10-04

## Scope and status

Continue from the qualified native document checkpoint `14f1a19`.
Proposed [ADR 0057](../../docs/adr/0057-prove-elf-loading-of-the-native-document-contract.md)
implements the next comparison: the same bounded document client through
native flat spans and a deployment parsed by an independent ring-3 ELF loader.
Both routes use the native `int 0xf4` execution/service personality. The
preceding [Native Document Log](2026-10-04%20Native%20Document%20Log.md) remains
the completed native baseline.

Integration checkpoint `a5853e6` was committed and pushed before the full
matrix completed. All **48 qualification checks** and both development-image
restoration jobs have now passed on **357 unchanged frozen non-Markdown
source/configuration files**. ADR 0057 remains Proposed with this bounded
proof qualified; no stable ABI is claimed.

The client still opens bytes through the independent RAM service, saves an
exact snapshot while making newer edits, and closes/reopens committed content
under a new handle. Before-commit and after-commit service failures preserve
an unknown pending save and dirty presentation. A fresh pair starts another
store and zeroed arenas. Saved bytes survive only inside the same live RAM
service; this does not add a filesystem, durable storage, editor owner
replacement, arbitrary ELF admission or full Antimatter/Electron.

## Artifact fit without widening admission

The full linked native document client and its 22,213 RX bytes are unchanged.
The original client has 4,224 all-zero RW file bytes. Keeping all those bytes
in the deployment would produce a 32,896-byte ELF before adding any loader
code, exceeding the loader's 32,768-byte RX allowance.

The new document-specific compactor retains the first 128 zero RW bytes and
the original 4,224-byte RW memory extent, eliding 4,096 verified-zero file
bytes. It independently reconstructs the virtual image and compares all
initialized bytes before accepting the artifact. The resulting deployment is
28,800 bytes; the qualified loader RX is 32,240 bytes, with 528 bytes of
remaining space. Growth beyond the limit must fail rather than silently widen
the profile. These are measured artifact-fit results.

The existing generic compactor, safe ELF parser, two-segment profile, 96-byte
inert plan and loader mapping bounds are unchanged. This transformation
preserves the virtual initialized image, not the original RW file geometry.
The application's `ANDS` startup descriptor at `0x801000` lies in the compact
route's zero-filled RW tail. The retained owner verifies every private RW,
stack and arena byte is zero before installing trusted startup data.

## Comparison and authority

Two loader tasks receive the complete immutable deployment slice, parse its
headers/table and validate extents/padding; payload bytes remain opaque. Only
the designated
role can offer one plan, and the kernel requires equality with its independent
immutable manifest. Wrong roles, bad copies, malformed plans, altered geometry
and replay cannot publish application tasks. Loader owners fully retire before
application allocation. The kernel checks copied extents and retained
authority; it does not parse ELF or implement document/save policy.

Each of the four native document scenarios runs through route zero (native)
then route one (ELF). Every pair retains the existing 66-frame, twelve-lease
arena profile. The separate loader pair keeps its 50-frame, ten-lease profile;
these lifetimes are sequential. Route comparison checks all semantic counters,
surface/document packet hashes, fault outcomes and cleanup, excluding only
bounded timer ticks. Both routes must also match the host's explicit native
document wire oracle independently. Matching wrong results cannot pass.

Normal routes compare every editor destination pixel before publishing the
ELF frame. Fault routes preserve the prior display. The final normal scenario
uses a distinct second document, whose reopened content is `const later = 2;`.
Outside-patch preservation and the whole-frame host oracle remain required.
The renderer is shared with the host pixel reference, so independent font
implementation is not claimed.

The strict transcript requires 224 ordered records: 21 records from the unchanged native
baseline oracle, readiness and loader receipts, eight route/scenario
receipts, 44 surface hashes, 148 document packet hashes and final success.
The old native proof retains its separate 123-record oracle unchanged.

## VM and emitted-code qualification

Final TCG/Intel and actual KVM/AMD runs passed at 96 MiB and 320 MiB, plus
separate host-quit captures. All six transcripts pass the exact 224-record
oracle. The ELF comparison's whole captures are byte-identical across the two
backends and to the current native document control on each backend. Visual
inspection confirms the final reopened `const later = 2;` document.

The loader trace records fifteen calls, one wrong-role denial, one bad copy,
eight invalid-plan results and one accepted plan. Both loader owners retire
before document allocation. Each normal route records 276 calls, 42 cooperative
switches, eight surfaces, four input messages, fourteen document requests,
thirteen replies, 102 arena copies and 24 published rows. Each fault route
records 107 calls, seventeen switches, three surfaces, two inputs, six requests,
four replies, thirty arena copies, two closed-route results and one discarded
pending save. Fault routes publish no rows.

The eight document runs retire sixteen document tasks; the preceding loader
pair retires two more. The unchanged native normal/fault baseline adds four
task lifetimes, making 22 across the complete proof. Every run restores its
resources. Each document pair starts with 65,536 zeroed arena bytes, totaling
524,288. Route equality includes all
four packet-hash channels and semantic outcomes; only bounded timer ticks are
excluded. The actual Antimatter save-store reference also passes with unchanged
sibling sources and reference harness.

Final TCG/KVM binaries exactly match the independently reviewed artifacts:

| Artifact | SHA-256 |
| --- | --- |
| ELF document kernel | `a7326e581c4207ce8633bb7bffe44b7892ed983edcb6ce19f15599600f73a91e` |
| Unchanged document client | `af05c5a7fa06e34e80b8dc374f01311a5c5975e409970fbb3b9e563cf5999bc4` |
| ELF loader | `63695e2c174a2efc48aed42bb7add32403689da05be091ede5fdff3411d97a92` |

Conservative emitted-code bounds for this ELF document kernel are 28,696 bytes
on the 65,536-byte bootstrap stack, or 30,007 including one complete timer
path. Gate/fault/timer bounds, including entry overhead, are 5,927/1,591/1,311
bytes of their 16,384-byte transition stacks. The unchanged client retains its
4,320-of-8,192-byte bound, with 2,408 live bytes at either service fault arm.
The exact new loader needs at most 912 of 8,192 user-stack bytes, including
panic paths. No unresolved admitted indirect edges or recursion cycles remain.
These bounds apply to the exact retained binaries and the fixed sole-BSP proof.
Kernel estimates cover admitted-return paths; fatal panic/Debug diagnostics,
dispatcher-fail formatting and explicit nonlocal enter/resume continuations
are outside that estimate. Client and loader bounds include panic paths.

The loader's 148 direct control transfers stay on inspected text boundaries;
there are no indirect calls/jumps or dynamic stack adjustments. It parses ELF
headers and the program table and validates extents/padding from the runtime
28,800-byte deployment slice; payload bytes remain opaque. The deployment is
non-SHF_EXEC `.rodata` within the same RX load, not a separately NX mapping. Kernel entry
review confirms CPU capture/restore, trusted descriptor installation after
complete zero checks, actual before/after-commit RAM observations and distinct
fault instruction sites. The kernel file ends at `0x140000` and resident image
at `0x1a2000`, within unchanged Threshold limits.

The rebuilt native control kernel is deliberately recorded separately. Its
client, symbols, writable data, BSS geometry and all text outside the document
initializer are unchanged. Naming the two zero data-source spans changes 229
initializer bytes and adds 32 bytes to its fixed stack frame, while all 94
control transfers remain identical. Seventeen panic source locations account
for eighteen changed read-only bytes. The raw native image has 255 changed
bytes: these 247 bytes plus eight valid Threshold checksum bytes. Debug
information also changes in the ELF. Conservative native-control bootstrap
bounds become 33,944 bytes synchronously and 35,255 with one timer; the prior
gate/fault/timer/client bounds remain valid. Both native control backends pass
and retain identical current kernels/images. This is explained build output,
not a claim that the prior whole native image stayed byte-identical.

## Regression and memory receipt

The final 48-job matrix includes every repository-required check, the new ELF
document comparison, prior native/ELF/Linux/Veil/shared-Veil/runtime/document
proofs, Ny, application CPU/copy/fault checks, desktop/workspace regressions and
both actual Antimatter host references. Host checks pass 491 kernel tests for
the document profile, 531 with all features, 195 tooling tests, seventeen
editor-contract tests, eleven document protocol/store/startup tests, six native
runtime tests and nine ELF adapter tests. All fourteen prior payloads remain
byte-identical: twelve shared-Veil blobs and the native runtime/document RX
clients. All seven preceding profile capture pairs remain equal across TCG/KVM.

Both normal and workspace development images are restored and byte-identical
to the preceding native-document checkpoint. The new profile remains excluded
from ordinary images. VirtualBox is installed, but this user's `/dev/vboxdrv`
is not readable or writable; no VirtualBox boot is claimed. Threshold was
unchanged.

Focused tests, artifact inspection and image construction passed before the
freeze. The first tooling/kernel lint and TCG preflight also passed. No failed
validation attempt or memory-pressure abort occurred in this slice. Builds and
QEMU jobs ran serially with two compiler jobs and one test thread, under
`MemoryHigh=4G`, `MemoryMax=6G` and `MemorySwapMax=512M`. A two-second monitor
stops work on a new OOM event or host available memory below 4 GiB.

No host or cgroup OOM occurred. The final matrix peaked at
4,296,990,720 bytes (4.002 GiB),
with at least 22,548,914,176 bytes
(21.000 GiB) of host memory available.
It recorded 4,144 soft-limit events and zero
hard-limit or OOM events. Soft throttling did occur; a retained near-limit
sample was predominantly file cache. Across all attempts, the maximum bounded
scope peak was 4,296,990,720 bytes
and the lowest host available memory was
22,548,914,176 bytes.

| Image | SHA-256 |
| --- | --- |
| anomalous.img | `4c63b9cc1e86ed92aa582da2a1ecf61719f4dd7d6a74548ba0767114876a6762` |
| workspace-dev.img | `bd4a51a6e7a8ab9b6d9e4c1ce4df2112d2c21c85e70eeaa66b26b5e8419266ca` |
| native-document-test.img | `dfe76ea1e95c0b03e0ad021148e22a3120125c769f18d20eb06313e82fe9144b` |
| elf-document-test.img | `2db62c7f6b9d4dfa8ca110dfda3c6978c10688779aa800cae92c4269b95004b7` |

Evidence is retained under
an external validation directory (not distributed).
`final-receipt.json` checks source freeze, exact matrix coverage, all results,
reviewed artifact identity, current native-control image differences, prior
payload/capture equality, restored images and actual Antimatter source hashes.
`matrix-final-results.json`, `restore-final-results.json`,
`memory-all-runs.json`, `stack-review.txt`, `critical-entry-review.txt`,
`loader-stack-review.txt`, `native-baseline-image-review.txt`, retained
logs/binaries and `evidence.sha256` preserve the supporting evidence. Generated
binaries, images and local receipts remain outside Git.

Commands are `make elf-document-inspect`, `make elf-document-image` and
`make elf-document-smoke`.

## Next gate

The next product increment retains the document service across editor owner
replacement, then adds scoped filesystem persistence with explicit durability
semantics. Ny remains the official language direction; HAG, Windows/macOS
personalities and a full Antimatter/Electron port remain separate work.
