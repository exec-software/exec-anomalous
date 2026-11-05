# 2026-10-15 NIL Runtime into Anomalous — Handoff

Purpose: hand the *runtime* side of "NIL on Anomalous" to the runtime owner
(Claude). The division of labor we agreed:

- **Runtime owner:** implement the build so a NIL edition (Matrix) actually runs
  on the Anomalous kernel. This is the first real consumer of the ABI, so ABI
  corrections are expected and welcome.
- **Matrix side (me):** keep writing Matrix code (Archon), producing real NIL
  editions that act as the workload and the test input.

This complements `2026-10-15 Archon on Anomalous Build Path Note.md` (the gap
analysis) and `docs/adr/0092-run-nil-on-anomalous-with-a-native-interpreter.md`
(the decision). Read those two first; this note is the implementer-facing
summary with exact numbers and the open ABI questions.

## The decision in one line

ADR 0092 selects the **NIL interpreter as the near-term execution route** for
NIL/Matrix on Anomalous: build the interpreter as a bounded native Anomalous
task that targets the Anomalous ABI, so a NIL edition is *interpreted* rather
than compiled ahead-of-time per program. Per-program AOT native compilation is
deferred (noted in the ADR).

VM here means the NIL interpreter (`nyinterpreter`), not a hypervisor.

## The workload, concretely

Archon builds to a textual NIL edition today:

```
# NIL Textual Module
module root

# String constants
const S1 = "Archon: hello from the shared Anomalous runtime"

# declaration_line 11
function main -> i32:
    PUSH_CONST_STR S1  ; line 12
    CALL         println 1  ; line 12
    PUSH_CONST   0  ; line 13
    CAST         i32  ; line 13
    RET           ; line 13
end function
```

Sizes of editions already built (`archon/build/`):

| edition | bytes |
| --- | --- |
| `hello.nil` | 334 |
| `model-smoke.nil` | 327,786 |
| `engine-smoke.nil` | 343,031 |
| `scanner-cli.nil` | 480,126 |

Build/run contract (host, today):

```sh
# from archon/
./runtime/nycompiler --no-bundle -I runtime/nystd -I src \
    --output build/scanner-cli.nil src/archon/cli/scanner-cli.mat
./runtime/nyinterpreter build/scanner-cli.nil testdata/insecure.mat
```

## What the interpreter is

- `runtime/nyinterpreter`: ELF 64-bit **PIE, dynamically linked**,
  `/lib64/ld-linux-x86-64.so.2`, x86-64, **1,131,648 bytes** (unstripped).
- Execution core is the **C++ `ny::VM`** in the Ny source tree
  (the historical Ny VM implementation); the command is a thin
  historical interpreter front end (CMake target `nyinterpreter`).
  See Ny decision `notebook/decisions/0002-nyinterpreter-is-the-runtime-command.md`.
- It uses a dynamic linker, hosted allocation, TLS and OS services — exactly the
  composition ADR 0049 says would "silently exceed the qualified kernel profile".

## What the ABI/kernel offers today (the "first real test")

**Kernel native tasks.** First-party native programs are freestanding `no_std`
**Rust** (`x86_64-unknown-none`) images with fixed low-address geometry and no
GOT/PLT/imports. Artifact geometry (`tools/xtask/src/native_application_artifact.rs`):

- `image_base = 0x400000`, `data_base = 0x800000`
- `image_limit = 8 * 4096` (32 KiB RX), `data_limit = 2 * 4096` (8 KiB RW)

**Native runtime startup** (`crates/anomalous-native-runtime/src/startup.rs`):

- descriptor is **128 bytes**, magic `ANRT`, version bytes `[1, 0, 128, 0]`
- `role <= 1`, `scenario <= 3`
- fixed **32 KiB (32768) arena**, page-aligned, nonzero base below `0x8000_0000_0000`
- `outbound_handle` and `inbound_handle`: distinct, nonzero
- `document`: at most **31 printable ASCII** bytes (not a general program image)

**Arena** (`crates/anomalous-native-runtime/src/arena.rs`): bounded bump
allocator over the 32 KiB, aligned, disjoint, no zeroing, exhaustion-safe.

**ELF adapter** (`crates/anomalous-elf-adapter/src/lib.rs`): accepts a
**sectionless** ELF64 deployment subset ("ANEL profile") only —
`image_limit = 65536` (64 KiB), page `4096`, `rx_base = 0x400000`,
`rw_base = 0x800000`, `rx_limit = 8 pages`, `rw_limit = 2 pages`, fixed entry,
zero padding, no allocation, no implied authority.

**Control plane ABI** (`crates/anomalous-abi/src/lib.rs`): invocation magic
`ANOM`, `abi_major = 0`, `abi_minor = 1`. The crate says plainly: "The ABI is not
stable while Anomalous is pre-alpha."

**Existing bounded Ny route.** ADR 0049 qualifies one *freestanding Ny object*
(source → NIL → retained object → opaque embedding wrapper → kernel payload) with
no hosted allocator/TLS/dynamic linker/GOT/PLT/unwinder and every import
inspected. It explicitly does **not** qualify the hosted Ny runtime or a general
compiler target. That is the AOT path we deferred.

## Where the ABI is very likely too small (the corrections)

These are the reasons the user expects to "correct the ABI":

1. **Image size.** The bounded image limit is 64 KiB (adapter) / 32 KiB RX
   (native application). Even `hello.nil` fits; **`scanner-cli.nil` is ~469 KiB**,
   and `model/engine` editions are ~320 KiB. A real NIL edition plus interpreter
   text will not fit the current fixed geometry.
2. **Memory.** The runtime arena is a fixed **32 KiB** and the RW window is
   8 KiB. An interpreter needs substantially more working memory (edition
   storage, values, strings, call stack) than either.
3. **Startup contract.** `ANRT` carries a role, a scenario, a 32 KiB arena, two
   handles and a ≤31-byte document — not "here is a program image and its
   argv". An interpreter needs an edition source, an `argv`-equivalent, and a
   way to return an exit status.
4. **Services.** The interpreter needs console output, bounded text/file reads,
   time, and (for Archon) reading files named on the command line. Today those
   cross the supervised boundary as explicit services (semantic control plane,
   ADR 0005/0018/0019); the host libc is not available. Console/file/time are
   the concrete services to land first.
5. **Handles.** Two handles (inbound/outbound) is a fixed shape; multiple
   services may need more, or a single multiplexed channel.

## Build options for the runtime side (my read, for your judgment)

- **A. Bounded first-party interpreter (Rust, `no_std`).** Reimplement the
  admitted NIL subset in Rust for Anomalous, validated differentially against
  the C++ `ny::VM` as oracle. No C++ toolchain needed; fits the project's
  bounded-subset style; but a second VM can diverge, so the oracle matters.
- **B. Freestanding build of the existing C++ `ny::VM`.** Reuses the real
  interpreter, but needs a no-hosted-libc/no-TLS C++ path that does not exist,
  plus an explicit compatible toolchain build. Likely blocked on toolchain.
- **C. Interpreter as a powered/hosted vessel.** Give the interpreter a richer
  profile than the bounded native task (real allocator, more memory, more
  services). This is where ABI/authority corrections concentrate.
- **D. Extend the ADR 0049 object route into a general NIL loader.** Runs
  editions without the interpreter, but is per-program AOT — the flexibility we
  chose against.

My expectation: a combination of **A** (interpreter core) with **C** (a properly
sized, service-bearing profile) is the realistic near-term shape; **B** is
attractive if a C++ freestanding path is actually available.

## Open questions for the runtime owner

**Answered 2026-10-15.** Full reasoning in
`2026-10-15 NIL Runtime into Anomalous — Runtime Owner Reply.md`; the decision
that question 6 turns on is recorded in
`docs/adr/0093-carry-a-semantic-manifest-in-the-program-image.md`. Summaries
below, with the reply as the authority.

1. Do we build the interpreter as a bigger **native task profile**, or introduce
   a new "powered vessel" profile? (This is the main ABI-shape decision.)
   → **A new profile.** The bounded 32 KiB task's value is that it is provable;
   widening it to fit an interpreter destroys what it exists for. Add a second
   profile beside it and leave the first untouched.
2. Edition delivery: embed the NIL in the image, or read it through a file
   service? (Affects image size and the file-service contract.)
   → **File service.** Embedding makes every program a fresh image, which is the
   one thing this route was chosen to avoid, and 469 KiB does not fit anyway.
3. Startup: extend `ANRT`, or add a new descriptor for program images with
   `argv` + exit status?
   → **A new descriptor.** `ANRT` is 128 bytes with tight invariants already in
   the field; extending it changes every existing validator for one consumer.
4. Services: which land first — console write, bounded file/text read, time?
   Multiplexed on one handle or several?
   → **Console write, then bounded file read, then time** — the order that walks
   `hello.nil` to `scanner-cli.nil`. **Separate handles** first; multiplexing
   needs a framing protocol that is itself new surface to qualify.
5. Rust `no_std` reimplementation vs. C++ freestanding build (option A vs B).
   → **A (Rust), and the toolchain is not the reason.** `vm.cpp` is 6,330 lines
   using `<filesystem>`, `<fstream>`, `<iostream>`, `<thread>` and `<locale>`,
   with 207 `throw` sites; B is a rewrite either way, so it should be a rewrite
   in the language the kernel is already proved in. The divergence risk gets a
   concrete gate: a second VM ships when it passes the `matrix.execution`
   corpus — 32 fixtures with exact output, three comparing emitted NIL schemas
   against hand-written Ny twins — unmodified.
6. Must a NIL edition be authenticated/verified before it runs, and who holds
   that authority?
   → **Yes.** ADR 0093: the edition carries a manifest of authored intent and a
   reachable authority surface *derived from the artifact rather than declared*,
   and the loader runs the intersection of that with the interpreter's ceiling,
   refusing outside it before execution rather than at the call.

### One correction of emphasis

The five corrections above are the sizing half and they all hold. The half that
is not sizing is that **the image says nothing about itself**: the semantic model
is entirely at the invocation boundary, while the image side carries fixed
geometry, a role, a scenario, two handles and 31 bytes of document, and the ELF
adapter states it conveys "no implied authority".

The interpreter is what makes that urgent rather than eventual. **An interpreter
is a generic image** — the same bytes run every program — so nothing learned by
inspecting it describes the workload. That is why question 6 turned out to be
the load-bearing one rather than the last one.

Two facts verified against the current compiler make it tractable: a Matrix
`context "..."` annotation already reaches the artifact as `@ai_context(...)`,
and a NIL edition already names every intrinsic its linked code can reach — a
file-reading page yields `file`, `path`, `console` in 277,615 bytes where an
arithmetic page yields no intrinsics at all in 846.

## Environment facts (verified in this checkout)

- `cargo`/`rustc` **1.97.0-nightly** (2026-05); toolchain override
  `nightly-2026-05-07-x86_64-unknown-linux-gnu`; target `x86_64-unknown-none`
  installed.
- `qemu-system-x86_64`, `VirtualBox`, `VBoxManage` all present.
- Relevant xtask targets: `native-application-image/-smoke`,
  `ny-object-image/-smoke`, `application-profile-image/-smoke`, plus the many
  `*-smoke` oracles under `tools/xtask/src/`.

## What I'll do meanwhile

Keep building Archon in Matrix so there is a real, growing NIL workload and a
concrete edition to run on the kernel as soon as the runtime path exists. I will
not depend on any runtime/ABI decision until it is recorded.
