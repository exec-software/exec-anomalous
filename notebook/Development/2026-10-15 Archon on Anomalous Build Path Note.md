# 2026-10-15 Archon on Anomalous Build Path Note

A working note on the precursor the user flagged: how would Archon — a Matrix
program — actually run on the Anomalous kernel so it can be exercised in the VM?
This records what exists today, not a decision. Any of the options below is a
new platform-level abstraction or stable format, so implementing one requires an
accepted ADR before code depends on it.

See also `2026-10-15 NIL Runtime into Anomalous Handoff.md`, the implementer-
facing summary (exact ABI numbers, likely ABI corrections, and open questions)
for the runtime owner.

## What the toolchain emits today

`runtime/nycompiler` (an unmodified Ny `nycompiler 1.0.0-rc.1`) can produce:

- a bundled executable (default), or a standalone **NIL** file with `--no-bundle`
  (textual, unless `--emit-binary-nil`);
- a self-contained **NIL library** with `--emit-library`;
- a **native Linux ELF** with `--native` (optional LLVM backend, linked with the
  host `c++`);
- **NyData** JSON via `--to-nydata` / `--to-json`;
- context JSON from an artifact via `--inspect-context`.

There is **no `--target anomalous`** and no Anomalous-specific output mode. The
`--no-bundle` NIL file is the artifact Archon produces today, and it runs under
`runtime/nyinterpreter`.

## Two gaps to running on the kernel

### 1. The kernel has no NIL/ny interpreter

Nothing under `kernel/src` references NIL, `nycompiler`, or an interpreter.
`crates/anomalous-native-runtime` starts a bespoke native task, not a NIL
edition: its startup descriptor is 128 bytes with magic `ANRT`, a role, a
scenario, a 32 KiB arena, outbound/inbound handles, and a ≤31-byte printable
"document". That is a fixed native contract, not a general program loader.

So a NIL file cannot be handed to the kernel as it stands. Options, each an
ADR-sized decision:

- **A. Interpreter vessel.** Run `nyinterpreter` as a supervised user task (a
  Linux image through the Linux veil per ADR 0073, or a first-party native
  build) and feed it the NIL edition. Reuses the existing interpreter; couples
  Archon to the interpreter's own platform support.
- **B. Native through the deployment subset.** Compile Archon with `--native`
  and route it through `anomalous-elf-adapter` + `anomalous-native-runtime`.
  This is the "target the Anomalous ABI first" direction, but see gap 2.
- **C. NIL loader in the kernel or a trusted vessel.** Define a bounded NIL
  loader/verifier that admits an edition and runs it under the semantic control
  plane. The largest new-surface option.

### 2. The native backend is not yet the Anomalous deployment subset

`nycompiler --native` produces a normal, dynamically linked host Linux ELF
(interpreter `/lib64/ld-linux-x86-64.so.2`). `anomalous-elf-adapter` accepts a
different thing: a **sectionless** ELF64 deployment subset ("ANEL profile"),
image ≤ 64 KiB, fixed RX/RW geometry, zero padding, no allocation and no implied
authority. The two are not the same format.

The native backend is also incomplete for Archon's needs. Compiling the CLI
natively fails on the first I/O intrinsic:

```
[NativeCodegen] unsupported native annotation '@intrinsic("file.read_text")'
```

The Matrix guide says native support is "provider-specific"; file I/O is not a
supported provider yet. A pure-computation page (the hello page) does compile
and run natively on the host, so the backend works for a subset.

## Consequence for the roadmap

- **Decided (ADR 0092).** The NIL interpreter is the near-term execution route
  for NIL/Matrix programs on Anomalous. It will be built as a bounded native
  Anomalous task that targets the Anomalous ABI, so a NIL edition is interpreted
  rather than compiled per program. Per-program AOT native compilation is
  deferred with a note in that record.
- The interpreter route needs a bounded, freestanding build of the C++
  `ny::VM` (no hosted dynamic linker, allocator, TLS or unwinder), fixed
  low-address geometry, and its operating-system needs (console, bounded file/
  text reads, time) exposed as explicit services through the semantic control
  plane. Each of those is its own qualification.
- Archon's own work is unaffected by the hosting choice: the same finding model,
  rule catalog and CLI carry over. Only how the entry page is hosted moves.
- Archon stays host-side (interpreter) until that route is qualified; that is
  what Units 1–9 build on.

## Open questions

1. Is "run `nyinterpreter` as a vessel" acceptable, or does the project require a
   first-party native Archon build with no interpreter dependency?
2. Would the native backend gain the intrinsics Archon needs (argv, file read,
   console write) as part of executing-readiness work, or as an Archon-driven
   extension?
3. Does the NIL edition need to be authenticated/verified before the kernel or a
   vessel will run it, and who holds that authority?
