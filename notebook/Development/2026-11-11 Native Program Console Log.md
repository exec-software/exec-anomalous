# Native program console transport

Date: 2026-11-11

## Scope

Proposed [ADR 0100](../../docs/adr/0100-qualify-native-program-console-transport.md)
connects the ANPG program profile's console grant to an actual bounded native
service. The existing named profile remains one task, one RX page, a copied
256-byte descriptor, and a 64 KiB arena. Two fresh lifetimes use arena bases
`0x804000` and `0x818000`; one grants the console and the other grants none.

The current increment deliberately executes no NIL edition. The
[Runtime Owner Reply](2026-10-15%20NIL%20Runtime%20into%20Anomalous%20%E2%80%94%20Runtime%20Owner%20Reply.md)
selects a bounded Rust `no_std` interpreter, with the existing execution corpus
as its acceptance gate, and separate services in the order console, bounded file
read, then time. A shared hosted C++ VM is not a prerequisite. Edition delivery
and [semantic-manifest admission](../../docs/adr/0093-carry-a-semantic-manifest-in-the-program-image.md)
remain prerequisites for claiming an admitted interpreted application.

## Integration baseline

The reviewable change is isolated on `codex/native-program-console`, based on
upstream `main` at `87e365d`. That baseline already includes Ny vessel input,
visible runtime-failure handling, and a yield that leaves and resumes the
vessel's address space. Its reported `peer_ran = false` limit remains intact;
this console increment does not claim a new peer-scheduling proof.

The baseline also includes the Linux loader-input re-pin in `0419f1e`. Those
changed package bytes and geometry belong to the existing upstream baseline,
not this console service. Only the console slice and its documentation are
carried into this worktree. The original checkout's unrelated working notes,
ELF host tests, Eidolon changes and handwritten interpreter are not incorporated.

## Implemented boundary

The experimental program gate uses `int 0xf4`. Console operation 1 receives the
raw handle in RDI, user address in RSI, byte length in RDX and sequence in RCX.
It returns status in RAX and committed sequence/length in RCX/RDX. Status values
are `ok = 0`, `denied = 2`, `bad_address = 3`, `invalid = 4` and `closed = 5`.
They retain the native packet conventions and are separate from `rite_status`
and the `int 0xf3` supervised-vessel gate.

The kernel's [console state](../../kernel/src/arch/x86_64/program_console.rs)
holds the controller-selected live task identity, optional raw handle, arena
bounds and next sequence. Descriptor contents are launch data and cannot replace
this retained binding. The profile checks the current task and complete frame,
then copies the complete admitted request range from the arena through
[owned execution](../../kernel/src/arch/x86_64/owned_execution.rs). A
4,096-byte BSS scratch buffer avoids putting the maximum payload on the
exception stack. It is logically cleared before and after every returning call.

The complete copy must pass the shared UTF-8 and control-character rule before
publication. Only LF and tab are allowed controls. Publication is synchronous;
the publisher must complete or never return. Sequence and byte accounting
advance afterward, so all rejected requests leave both output and sequence
unchanged. This is not a recoverable transaction for a partially failing sink.
Exit closes the binding before task and mapping retirement.

The [client](../../vessels/anomalous-native-program-client/src/main.rs) checks
actual status, byte count and sequence replies through its assembly stub. It
places the first text buffer at arena offset 4,095 to exercise an unaligned
copy across a real page boundary. Raw probes bypass the client wrapper to test
kernel rejection of malformed requests and missing authority.

## Verified native evidence

The first lifetime commits exactly these two newline-terminated UTF-8 messages:

```text
Native console: ready — ✓
Native console: reply checked
```

Each contains 30 bytes including LF. The granted lifetime makes sixteen console
attempts: two writes and fourteen rejections. The ungranted lifetime rejects
both attempts, including a previously valid numeric handle. Total gate calls
are eighteen and four, including startup and exit. Neither lifetime reports an
invalid gate operation or an expected fault.

The [transcript oracle](../../tools/xtask/src/native_program_smoke.rs) retains
the twelve structured event names. It requires both exact raw lines, once and
in order, after the eight boot/interrupt records and before `ready`. Native
output and every new accounting field are independently specified. The final
record requires two writes, sixteen rejected requests, 60 committed bytes,
verified console output, 131,072 observed zeroed bytes, restored resources and
revoked authority. `edition_mapped` remains false and `editions_executed` remains
zero.

The native task still owns 25 data frames, nine table frames and six leases per
lifetime. Existing cleanup checks cover scheduler/task/circle state, eager CPU
ownership, transition storage, the physical alias and allocator restoration.
The new binding is closed and its scratch clear when each lifetime ends.

## Final validation

The final verifier passed at `2026-11-11T23:15:38Z`: all 27 jobs passed on
503 unchanged source files. The matrix contains two focused suites, 24 standard
targets and an additional KVM native-program proof. Host results were:

| Suite | Passing tests |
| --- | ---: |
| Kernel, default features | 643 |
| Kernel, all features | 717 |
| xtask | 398 |
| Native runtime | 30 |
| Focused console state | 7 |
| Focused native transcript oracle | 8 |

All four native boots passed the exact console contract above, including twelve
structured records, two raw lines, 60 committed bytes, sixteen rejections,
131,072 observed zeroed arena bytes and no edition execution. The retained
16 MiB TCG and KVM images are byte-identical.

| Backend / CPU | RAM | Guest vendor | Restored free frames |
| --- | ---: | --- | ---: |
| TCG / Nehalem | 96 MiB | GenuineIntel | 24,172 |
| TCG / Nehalem | 320 MiB | GenuineIntel | 81,516 |
| KVM / host | 96 MiB | AuthenticAMD | 24,172 |
| KVM / host | 320 MiB | AuthenticAMD | 81,516 |

The TCG backend and CPU selection are established by the frozen xtask source;
its command arrays were not separately retained. Both KVM command arrays were
recorded. The verifier also confirmed that the original checkout's tracked diff
and all seven unrelated-file hashes match their initial snapshots.

### Retained attempts

A preliminary native program-console proof passed in the original development
checkout before the slice was isolated onto the current upstream baseline.
That result is development evidence, not qualification of the final source or
the upstream Ny and Linux composition.

The initial `make check` attempt in that checkout failed because the console
module's test configuration imported an optional runtime dependency without
enabling it. Restricting the module to `native_program_test` fixed that before
the final source freeze. The isolated baseline's complete `make check` passed.

The first KVM attempt retained the TCG `Nehalem` CPU model. On the AMD host it
did not expose `xsaveerptr`, so admission correctly refused execution with
`application_cpu_amd_error_pointers`. The retained artifact wrapper was changed
to `-cpu host`; the kernel and frozen source were unchanged. Both corrected KVM
runs passed. Neither failure is counted as a successful final job.

### Reproduction commands and artifacts

Run the same serial matrix from the repository root:

```sh
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
cargo test -p kernel --lib --features native_program_test program_console
cargo test -p xtask native_program_smoke
for target in \
    check smoke fault-smoke circle-smoke control-smoke rite-smoke \
    scheduler-smoke supervision-smoke shell-vessel-smoke memory-smoke \
    interrupt-smoke input-wait-smoke shell-smoke \
    semantic-surface-transport-smoke semantic-presentation-smoke normal-exclusion \
    native-program-smoke native-runtime-smoke native-application-smoke \
    elf-application-smoke linux-application-smoke linux-loader-entry-smoke \
    workspace-smoke ny-vessel-smoke
do
    make "$target" || exit
done
```

For the additional KVM job, set `console_evidence` to the retained evidence
directory and run
`PATH="$console_evidence/kvm-bin:$PATH" make native-program-smoke`.
Its `qemu-system-x86_64` wrapper prepends `-accel kvm`, replaces the existing
`-cpu Nehalem` with `-cpu host`, records the command and executes QEMU. The
effective command for each of `96M` and `320M` is:

```sh
qemu-system-x86_64 -accel kvm -machine q35 -m "$ram" \
    -drive format=raw,file=build/native-program-test.img -no-reboot \
    -serial stdio -cpu host -display none -monitor none \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04
```

The retained evidence bundle is named `program-console-2026-11-11`; generated
images and logs are not committed. Its key artifact identities are:

| Artifact | SHA-256 |
| --- | --- |
| `summary.json` | `4d092e9f028e1beeef04386fafbc3651bc1bf1b9e34f5550ba60e3e7ef14dd94` |
| `source-sha256.json` | `faa903b4a49ecfdf2584161401f68e8c45a6fbe45eb058bf4537d9a3d15bf555` |
| `native-program-smoke.img` and `native-program-smoke-kvm.img` | `08f42dc3b65906c22254b647006f0b2302f9d3013e1b3d14bcf62f724ec4aebd` |
| `native-program-smoke.log` | `2e50333533f3ce4c53e723d48213594d44affdc65430d0d08ae9eba014975ed6` |
| `native-program-smoke-kvm-attempt-2.log` | `548b31a5a7af303e7b53290e5d65fafb0940fd97737608a04fe496262e253b00` |
| `check-attempt-2.log` | `08e8ee27ea65d3446be55c0e30075e8956a23b4724b3abf25a75ea6e52597fda` |
| `linux-loader-entry-smoke.log` | `e4645d6bcd2a1b2ac941714fbb17a528e461c2afbc3c22992258a5a206ae5f2c` |
| `kvm-invocations.jsonl` | `bc5f71eb120b0fb977f8607850ea804eb086d881bf50758f00ebdb3bc5f614ab` |
| `kvm-bin/qemu-system-x86_64` | `5c71ceb63e6d852c82c778b6353598ae803917b747b92fb82a13d5dfc9f08d1d` |

`results.json` retains all attempts, including the failed `check.log` and
`native-program-smoke-kvm.log`. The complete command list and per-job results
are also in `summary.json`. ADR 0100 remains Proposed; measured qualification
does not imply ratification of a stable application ABI.

## Preserved work and remaining gates

The pre-existing untracked working file
`vessels/anomalous-native-program-client/src/edition_interpreter.rs` is preserved
but is not linked into this client. Its permissive parser and the
absence of manifest enforcement or differential corpus qualification do not
justify an interpreter-execution claim. Unrelated working notes, ELF host tests
and Eidolon changes remain outside this increment.

Next native work is bounded edition delivery and semantic-manifest admission,
followed by the selected Rust interpreter's qualified execution. This console
proof establishes only the first service it can use.

Fresh `linux-loader-entry-smoke` evidence at both 96 MiB and 320 MiB observes
sequence 12 as `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`. The package
re-pin changed the historical `0x198000` length to `0x189000`: 393 pages rather
than 408, with the other arguments unchanged. The preceding sequence-11 call
`mmap(0, 0x212d90, 1, 0x802, 3, 0)` returned base `0x100000000`.

The sequence-12 `kernel.linux_loader_entry_test.boundary` events occur inside
the interpreter at syscall instruction `0x60025d2a`, with return RIP
`0x60025d2c`; main entry is `0x400019f0`. They report
`returned_to_caller = false`, `linux_exit_observed = false` and verified syscall
bytes. The `terminal_copy` events preserve the complete pending frame, and
`passed` reports twelve raw calls, eleven returns and restored resources. The
normal-lifetime evidence is at lines 55–68 and 290–303 of the retained log.

[ADR 0098](../../docs/adr/0098-model-source-backed-executable-replacement.md)
models the historical request only. The next Linux mechanism is live
source-backed executable permission replacement for the current request over
retained ownership. It requires common permission verification, authenticated
mutation/reply, actual RX execution, protected writes/NX neighbours and cleanup
through partial failures. No RX replacement was performed by this run; libc
startup, main `true` entry, Linux exit and Electron/Antimatter execution remain
unproved.
