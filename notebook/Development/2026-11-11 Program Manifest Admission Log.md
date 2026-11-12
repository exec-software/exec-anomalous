# Program manifest admission

Date: 2026-11-11

Status: Accepted bounded host checkpoint; all 25 final jobs passed.

## Baseline and purpose

The increment begins at `914d942`, the qualified
[native edition-delivery checkpoint](2026-11-11%20Native%20Program%20Edition%20Read%20Log.md).
That checkpoint delivers synthetic bytes through a task-bound source handle and
executes no edition. Its thirty passing jobs do not qualify this new parser,
format, dependency or admission policy.

Accepted [ADR 0102](../../docs/adr/0102-admit-bounded-program-semantic-manifests.md)
implements a host-only prerequisite to accepted
[ADR 0093](../../docs/adr/0093-carry-a-semantic-manifest-in-the-program-image.md):
a reproducible semantic manifest bound to exact edition bytes, checked against
an explicit interpreter workload ceiling. Native loader integration follows
after the shared component is qualified; the kernel and native delivery client
remain unchanged in this increment.

## Audit findings driving the scope

An intrinsic-name scan is insufficient. NIL can express effects through static
builtins such as `println`, direct console/time operations, assembly and several
interop libraries. Annotation-bearing `# function_annotation` and
`# declaration_annotation` records cannot be ignored as ordinary comments.
Unsupported syntax must therefore cause refusal, not disappear from derivation.

The retained SDK `hello.nil` inspected during design has a `CALL println 1`
but no authored context. It cannot become a compliant positive manifest fixture
by deriving intent from its filename. Positive qualification inputs are explicitly
hand-authored textual NIL with authored intent; complete compiler outputs are
retained separately as negative fixtures.

The current compiler can preserve ordered repeated annotations and legacy
composite forms. It can also retain already-decoded string content. The first
profile intentionally supports exactly one plain `@ai_context` string on `main`
and refuses escaped, encoded, composite or misplaced forms. This is an explicit
subset limit; it must not be described as complete NIL annotation support.

Fresh compiler-produced sources with context revealed another dependency:
`@ai_context` also emits `__nilaix_register_ai_context_hash`, a `void` helper
containing `INTEROP_CALL nilaix register_ai_context_internal 5 void`. It is
outside the accepted initial grammar and cannot be dropped from the authority
union. Inspection found no supported CLI release mode that suppresses it. The
bounded profile therefore refuses the complete compiler outputs, with no
helper-stripping or rewrite pass. Source, outputs and provenance are retained
in the `program-manifest-2026-11-11/sdk-fixtures` bundle and the
[fixture provenance note](../../crates/anomalous-program-manifest/fixtures/README.md).
The production compiler-to-admitted-edition bridge remains unsupported; it is
not resolved by the positive hand-authored fixtures.

The existing ANPG `file_read` bit names only the loader's retained edition
delivery service. It is separate from the effects the edition may perform.
Conflating those masks would either prevent a zero-authority edition from being
loaded or misrepresent broad file operations as read-only authority.

## Components and wire contract

`anomalous-program-manifest` is a separate `no_std`, unsafe-free userspace crate.
Host `xtask` pack/inspect operations use its shared parser, derivation, codec and
admission path. A private borrowed `admitted_edition` result binds the exact
immutable checked bytes; claimed mask/digest/intent fields cannot construct it.
No kernel NIL parser or manifest policy is introduced.

The experimental unsigned `.re` envelope has a 64-byte `ANPROG\r\n` header,
a required 96-byte `ANSMAN\r\n` manifest header, 1–512 bytes of plain UTF-8
intent and 1–65,536 exact edition bytes. Both versions and derivation
profiles are exactly 1. All integers are little-endian. Offsets and lengths
must describe contiguous sections in that order, with no gaps, padding,
overlap, unknown sections or trailing bytes. Flags and reserved bytes are zero.
The maximum envelope extent is 66,208 bytes; parser structure has further
explicit bounds. ADR 0102 records each field offset.

SHA-256 covers the exact edition bytes, including their original annotations
and whitespace. It excludes the manifest and envelope. Admission independently
rederives the digest, intent and authority surface and requires exact equality.
This is content identity, not publisher authentication, an entity identity,
a signature or a grant. The container is not a completed Covenant artifact.

The selected hash dependency is RustCrypto `sha2 = 0.10.9`, pinned with default
features disabled and `force-soft` enabled. It is confined to the userspace
manifest crate and host tooling. Its feature selection is not native code-size,
stack or CPU-state qualification; there is no new hash code in a kernel payload
in this slice.

## Derivation and admission profile

The parser fully consumes an explicitly supported textual NIL subset. It accepts
bounded zero-argument `i32`/`i64` functions and explicitly
allowed instruction/call forms, scans every accepted body and refuses unsupported
forms. Direct user-function calls, dynamic calls/function values, unsupported
interop and assembly are outside the initial profile. Straight-line operations
undergo abstract integer/text value-kind and stack-shape checks, without
evaluation. A structurally admitted arithmetic operation may still fault when
eventually executed; this proof does not run it.

| Bound | Maximum |
| --- | ---: |
| Edition bytes, nonempty | 65,536 |
| Plain UTF-8 intent bytes, nonempty | 512 |
| Functions | 64 |
| String constants | 128 |
| Instructions across all bodies | 4,096 |
| ASCII identifier bytes | 128 |
| Locals per function | 128 |
| Abstract stack values per function | 256 |

Supported pure computations derive an empty surface; supported console effects
derive `console_write = 1 << 0`. Other effect families are unsupported. Exactly
one supported plain UTF-8 intent annotation must be attached to `main`; it has
no interior quote, backslash or control character. No producer may invent missing
intent or silently discard an unsupported annotation.

The embedding supplies an explicit known workload ceiling. Unknown ceiling bits
and derived services outside that ceiling are refused before admission returns.
The effective surface is the intersection, so a wider ceiling cannot add a
service the edition did not derive. The result supplies no kernel capability
and performs no effect by itself.

## Host receipts and publication

`program-manifest-pack` reports `host_package_validated`: it derives and checks
package consistency, and reports intent, required surface and hashes. Its
receipt has no ceiling or granted-services fields. `program-manifest-inspect`
requires an explicit `none` or `console_write` ceiling and reports
`host_policy_admitted`, including that ceiling and the effective services.
Both report `executed: false`; neither receipt is a kernel grant.

Input reads are byte-bounded snapshots in a trusted host namespace. Publication
assumes a trusted parent directory and cooperating writers. A complete, synced
temporary inode is linked without replacing an existing destination, then the
temporary name is removed. This establishes neither a hostile-filesystem/I/O
latency bound nor directory-fsync or power-loss durability. Validation refusal
publishes nothing. After publication, cleanup or receipt-write errors report
that the complete output was published and retained. The measured `/dev/full`
case exits with an error and leaves the exact complete arithmetic package.

## Final qualification

The retained `program-manifest-2026-11-11/qualification` bundle contains the fixed
job plan, all logs and receipts, independent CLI wire proof, frozen source hashes
and final verifier. Verification completed at `2026-11-12T01:34:08.246889Z`
(local November 11). All **25 jobs passed on their first attempts**, with no
memory/time guard failures: seven focused host checks, the 16 required targets,
`native-program-smoke` and `linux-loader-entry-smoke`. `make check` took 664.1
seconds. The focused checks include host and `x86_64-unknown-none` clippy with
warnings denied; they do not qualify a native interpreter payload.

| Suite | Passed |
| --- | ---: |
| Kernel default | 644 |
| Kernel all features | 727 |
| Tooling, full | 411 |
| Native runtime | 35 |
| Manifest, full and focused | 18 each |
| Manifest tooling, focused | 9 |

The verifier checked the exact set and hashes of **608 non-Markdown source
files**, unchanged throughout qualification. The source-manifest SHA-256 is
`3938aac13e98ede447797c97253c7669be1d25cdc067fc85fb6c4e38288d26fe`.
The final `summary.json` SHA-256 is
`f13273871689cf3e1b3729e615611e195a9e668ee94cb4a46b9d0a40cc97ca7d`;
it binds all successful logs, retained images, receipts and verifier artifacts.

The manifest proof accepted four positive fixtures (two retained hand-authored
editions and two inline editions), refused both complete compiler outputs, and
rejected **1,659 mutations**. The separate CLI proof recorded **20 commands**.
An independent Python encoder reproduced each package byte for byte. Cases
covered explicit ceiling admission/refusal, forged mask and identity, unknown
profile/surface, rehashed changed intent, compiler-output refusal, malformed
arguments, Unicode/space paths, refusing an existing output without changing it,
and the post-publication `/dev/full` receipt error. No temporary files remained.

| Hand-authored fixture | Edition bytes | Package bytes | Required/effective services under ceiling 1 |
| --- | ---: | ---: | --- |
| Arithmetic | 386 | 606 | 0 / 0 |
| Console | 319 | 536 | 1 / 1 (`console_write`) |

Arithmetic's 606-byte package SHA-256 is
`658ec75db85c2e5d48b74188fa4b3e716b591b22c65cdbead4f01c63416162ad`;
console's 536-byte package SHA-256 is
`7ce074a0d0aee408a9d19ed017ce91fb39cb3e5a6dc834884a1be1f7b70744c5`.
Edition identities and exact authored intent are retained with the receipts and
[fixture provenance](../../crates/anomalous-program-manifest/fixtures/README.md).
The SDK read-only context inspector accepted both hand-authored contexts. Its
retained provenance reports revision `241eb762e106079f3ac362f879c7a86d737948af`
with a dirty worktree; it is not evidence of a clean reproducible compiler build.
Neither context inspection nor host admission executed an edition.

The unchanged native client passed the TCG delivery regression at 96/320 MiB,
including three source-provisioning refusals and cleanup. Its 16,777,216-byte
image exactly matches the prior qualified edition checkpoint:
`e611655bf95c6fb4da0b402dd83dc875c3069788d2c299a80ba55d39bd54cb2a`.
No new KVM or VirtualBox run was made. Linux also passed at both RAM sizes and
remained stopped at normal request 12,
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`, after 11 caller returns.
That request did not return to the caller and no Linux exit was observed.

The preliminary tooling-test attempt failed with Rust `E0530`: local bindings
shadowed the `directory` tuple struct. The corrected retry passed before the
source freeze. Its original exit-101 log remains in the preliminary bundle,
separate from the final 25 successful attempts; its SHA-256 is
`36699b2f58d05c2c5c8c655ce881e07c059ebe4c025a336aa5e0b65d152861b8`.
The original checkout's pre-existing tracked diff and seven unrelated file
hashes were separately confirmed unchanged.
The two complete compiler-output fixtures intentionally retain their emitted
trailing whitespace and final blank lines. Final patch whitespace checking
excludes those two hash-pinned files only; their bytes match the retained SDK
outputs exactly.

## Remaining native work

A subsequent native loader can read the same envelope through
the existing retained-source mechanism and use the same admission component
before interpreting any edition. The loader is trusted for its bootstrap
delivery and diagnostics; the edition's workload authority remains narrower.
Its real code, stack and arena requirements need measurement and an explicit
profile decision rather than an incidental expansion of the two-page client.

No interpreter corpus, NIL execution, general file access, new kernel grant,
entity authentication, source-to-admitted-edition compiler pipeline or Linux
execution milestone is claimed here. The
`matrix.execution` corpus remains the separate acceptance gate for the selected
Rust interpreter, alongside actual enforcement of the admitted workload surface.

The future Rust intrinsic dispatcher needs its own effect-enforcement proof.
Do not inherit an authority claim from the hosted C++ interop path: that path
also invokes FFI hot-reload machinery before intrinsic dispatch. Recognizing
`console.write` there is not proof of console-only execution under this manifest.
