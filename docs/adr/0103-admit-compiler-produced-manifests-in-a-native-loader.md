# ADR 0103: Admit compiler-produced manifests in a native loader

- Status: Accepted (bounded native admission checkpoint)
- Date: 2026-11-11
- Builds on: ADRs 0091, 0092, 0093, 0094, 0101 and 0102
- Scope: Bounded compiler-output packaging and native userspace admission; no edition execution.

## Context

[ADR 0101](0101-deliver-bounded-native-program-edition-bytes.md) qualifies one
immutable source delivered through a native task's edition handle.
[ADR 0102](0102-admit-bounded-program-semantic-manifests.md) qualifies a shared
host parser, unsigned program envelope and admission function. Its positive
fixtures are hand-authored because the default compiler emits foreign
context-registration helpers outside the admitted profile.

The next boundary must connect complete compiler outputs to that same admission
function inside a native task. Language parsing and workload policy remain
userspace responsibilities. The kernel needs no NIL grammar, manifest parser or
knowledge of authored intent to own sources, copy bytes and enforce live service
bindings.

## Decision

### Make compiler registration an explicit option

Use the compiler's opt-in `--no-context-registration` option when producing this
bounded program profile. It suppresses compiler-generated runtime registration
helpers and their associated test initializer while preserving authored
`@ai_context` annotations and explicit foreign or user effects. Default compiler
behavior remains enabled. The option applies per compilation to text and binary
output; discovery, inspection, analysis and library combinations reject the
inapplicable option.

Retain complete arithmetic, console and larger arithmetic outputs from compiler
commit `4dad0512dfd3fa4e351f2816196cb48956f5442e`, with source, command, compiler
identity and output hashes in the
[fixture provenance](../../crates/anomalous-program-manifest/fixtures/native/provenance.json).
Package those exact bytes without rewriting instructions or removing helpers
after compilation. Keep a complete default-compiler output containing the
external registration helper as a refusal fixture. An omitted helper is not
authority-free merely because its name looks like metadata.

The ADR 0102 parser and envelope limits remain unchanged. This connects selected
compiler outputs to admission; it does not admit arbitrary compiler output,
extend the NIL language profile or qualify binary NIL admission.

### Admit held bytes in the native loader

Add a default-off `native_manifest_test` feature and a separate
[`anomalous-native-manifest-loader`](../../vessels/anomalous-native-manifest-loader/src/main.rs)
vessel. The feature depends on `native_runtime_test`, without selecting the old
program proof or the Linux adapter. Reuse the retained one-task controller in
[`native_program_profile.rs`](../../kernel/src/arch/x86_64/native_program_profile.rs),
with fixture and report policy in
[`native_manifest_profile.rs`](../../kernel/src/arch/x86_64/native_manifest_profile.rs).

The loader copies the ANPG startup descriptor once, verifies its expected
location and shape, and reads the entire external package into its private
arena in requests of at most 4,096 bytes. Require exact successful sequence and
length replies. The package extent is from one through 66,208 bytes; report,
console scratch and package storage must have disjoint borrows. Hold the package
immutably while calling the same `anomalous-program-manifest::admit` used by host
inspection. Identity, derived authority and authored intent must match before
an `admitted_edition` witness exists.

The kernel has no dependency on the manifest crate. It provisions opaque bytes
and bootstrap services. Every fixture receives `console_write | file_read`,
mask `3`, so that the loader can fetch its package and report a permitted
diagnostic. A separate trusted ANPG argument supplies the workload ceiling,
either `ceiling=0` or `ceiling=1`; neither the package nor its declarations choose
this ceiling. A bootstrap delivery grant does not become workload authority.

Require an admitted witness at the helper that performs the success action.
Only a witness granting console write may emit the fixed loader marker
`Native manifest: console admitted\n`, with its native reply verified. An admitted
authority-free package and every refusal produce no console call. The marker is
loader behavior, not execution of the package's console instruction. Bootstrap
kernel grants are not dynamically narrowed by this userspace admission result.

### Use a separate measured mapping profile

Name the geometry `manifest_loader`; do not widen the previous program or native
application profiles. The inspected loader payload is 18,874 bytes, requiring
five RX pages. Admit its 20,480-byte padded payload at `0x400000`, two RW data
pages with the unchanged 256-byte ANPG descriptor at `0x801000`, a 128 KiB arena
and a 32 KiB stack. Arena bases `0x804000` and `0x828000` alternate between fresh
lifetimes. Keep the existing guard policy and stack top.

This profile owns 51 data frames: five code, two data, 32 arena, eight stack and
four transition frames. Nine table frames and six execution leases remain
separate from each source's data frames and bound lease. Generic five-page
payloads remain invalid: only the retained single-task manifest profile with
its exact region geometry admits that width. The original two-page program
client and eight-page native application retain their existing limits.

### Preserve the source and execution lifetime ordering

Use the existing configured sealed-source controller with ten exact fw_cfg item
names. Each trusted catalog entry independently bounds its length to 66,208
bytes and pins its own FNV value. Each malformed semantic package must have a
matching transport pin so that it reaches ring 3; a provisioning refusal cannot
stand in for a parser or policy refusal. FNV is a corruption check. The manifest's
SHA-256 is content identity. Neither authenticates a publisher.

Import and seal the source before creating the execution alias, since both
owners use the same high physical-alias slot. Finish synchronous preparation
and all allocator and mutable-state borrows before transferring to the guest,
as required by [ADR 0091](0091-release-allocator-borrows-before-userspace.md).
Retained BSS ownership and copied raw pointers bridge entry and continuation.
Reacquire ordinary allocator access during an authenticated read, preserving
the complete saved frame and requiring the nested-borrow denial.

On terminal return, restore bootstrap root, stack and CPU state, withdraw active
dispatch and revoke service bindings before cleanup. Reborrow the allocator,
retire execution, then finish the source through its independent cleanup alias.
Require scratch clearing and exact ownership restoration. Missing, short and
corrupted provisioning inputs must release any partial owner before their
attributed fatal refusal, without publishing a task.

### Keep diagnostics separate from authority

The loader writes the canonical 768-byte
[`manifest_report`](../../crates/anomalous-native-runtime/src/manifest_report.rs)
and terminates through operation 5. Accepted reports carry the witness's
identity, intent, required and granted surface. Refused reports carry a known
reason and zero all admission fields. Reserved bytes and execution flags remain
zero. The kernel checks canonical representation, configured ceiling and length,
actual read/write counts, receipts, closed bindings and lifecycle evidence.

Any task can construct report-shaped bytes. Decoding them creates no admission
witness, capability, signature or kernel grant. A separate host oracle checks
the expected semantic outcome and exact fields against retained fixture bytes.
Qualification therefore binds the inspected trusted loader's control flow,
native service evidence and independent expected result; it does not treat a
self-reported success bit as authority.

## Acceptance contract

Run ten fresh one-task lifetimes per successful boot, with four admissions, six
refusals and exactly one checked loader console write:

| Package | Workload ceiling | Required result |
| --- | ---: | --- |
| Compiler arithmetic | 0 | Admit, no required services |
| Same arithmetic | 1 | Admit, no required services |
| Compiler console | 1 | Admit console; one loader marker |
| Same console | 0 | Refuse `manifest_ceiling` |
| Compiler large arithmetic | 0 | Admit after three package reads |
| Changed edition with stale identity | 1 | Refuse `manifest_identity` |
| Understated authority | 1 | Refuse `manifest_mismatch` |
| Changed intent with updated edition identity | 1 | Refuse `manifest_mismatch` |
| Nonzero reserved encoding byte | 1 | Refuse `manifest_encoding` |
| Complete default-compiler registration helper | 1 | Refuse `manifest_unsupported` |

Require strict artifact inspection of entry, RX/data extents, startup and gate
machine bytes, plus a stack-size review. Preserve malformed geometry rejection
and previous profile goldens. Qualify compiler option behavior, complete-output
provenance, shared admission tests, canonical report rejection and host selector
precedence. Retain source and image hashes, failed attempts and all required and
affected regressions.

Require native runs at 96 and 320 MiB under QEMU TCG and host-CPU KVM, each with
exact readback, report and terminal evidence; separately require missing, short
and corrupt source refusals. The current external-source adapter is explicitly
QEMU fw_cfg. The repository's VirtualBox proof helper attaches a boot image but
has no corresponding package-provisioning adapter. Do not infer VirtualBox
unavailability or successful native admission from this backend scope.

All 24 final qualification jobs passed against 624 unchanged non-Markdown
source files. The exact same native admission image passed TCG and AMD KVM at
96 and 320 MiB, with three independent provisioning refusals per backend.
The existing delivery profile passed both backends and sizes; the Linux
regression preserved its unanswered sequence-12 request. The compiler bridge
passed eight focused tests. See the
[Native Manifest Loader Log](../../notebook/Development/2026-11-11%20Native%20Manifest%20Loader%20Log.md)
for artifact identities, emitted-code review, failed preliminary attempts and
the independent verifier. This acceptance does not include NIL execution.

## Remaining gates

This checkpoint executes a native loader and admits compiler-produced program
bytes. It executes no NIL edition. A bounded Rust interpreter must still qualify
against the required execution corpus and bind its dispatch and mediated
intrinsics to the admitted witness. Hosted FFI and hot-reload behavior do not
transfer into that qualification. Stable ABI, publisher authentication,
general file namespaces, broader compiler support and Linux application
completion remain separate work.
