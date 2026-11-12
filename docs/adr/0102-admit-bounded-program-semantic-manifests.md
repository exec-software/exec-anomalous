# ADR 0102: Admit bounded program semantic manifests

- Status: Accepted (bounded host checkpoint)
- Date: 2026-11-11
- Builds on: ADRs 0093 and 0101
- Scope: Host-only packaging, derivation and admission of a bounded NIL text profile.
- Qualification: All 25 final jobs passed; no native loader or interpreter execution is claimed.

## Context

[ADR 0093](0093-carry-a-semantic-manifest-in-the-program-image.md) requires an
artifact-derived authority surface, authored intent and artifact identity before
an edition runs. [ADR 0101](0101-deliver-bounded-native-program-edition-bytes.md)
qualifies delivery of immutable edition bytes to a native task. Its synthetic
binary fixture establishes no NIL semantics or manifest admission.

The next prerequisite is a reproducible artifact contract that can be shared by
host packaging and a future userspace loader. A search for `INTEROP_CALL` cannot
establish an authority bound: NIL also has builtin calls, direct console and time
operations, assembly, and several interop libraries. Intent can occur in special
annotation records. Ignoring unfamiliar records would silently understate what
an artifact contains.

Language parsing and admission policy belong in userspace. Kernel mechanisms
continue to enforce live task authority, immutable source ownership, bounded
copying and cleanup. Artifact identity is distinct from entity authentication
and actual grants: a matching digest neither identifies an author nor grants
access. The accepted assertion boundary in
[ADR 0023](0023-prove-bounded-authenticated-source-session-routing.md) likewise
does not turn public identifiers into authority.

## Decision

### Isolate the shared userspace component

Add `anomalous-program-manifest` as a separate `no_std`, unsafe-free crate.
Host `xtask` packaging and inspection use that same implementation for complete
artifact validation, derivation, canonical encoding and admission. The kernel,
native client, ANPG startup encoding and qualified two-page program geometry
remain unchanged in this slice.

The crate must not expose a constructor that treats a caller-supplied digest,
surface mask or intent as already verified. Successful admission returns a
private `admitted_edition` witness borrowing the exact immutable package/edition
bytes that were checked. That witness is a local program-validation result,
not a serialized capability, kernel grant, authenticated entity or signature.
The host reads one bounded snapshot and uses those held bytes throughout;
reopening a path after inspection cannot substitute another edition.

### Define one canonical unsigned envelope

Use an experimental `.re` program envelope containing a required manifest
followed by the exact edition bytes. This is a bounded precursor to the richer
artifact direction, not the complete Reliquary or Covenant format. All integer
fields are unsigned little-endian values. The outer header is exactly 64 bytes:

| Offset | Bytes | Meaning |
| ---: | ---: | --- |
| 0 | 8 | Magic `ANPROG\r\n` |
| 8 | 2 | Envelope version, exactly 1 |
| 10 | 2 | Header size, exactly 64 |
| 12 | 4 | Derivation profile, exactly 1 |
| 16 | 8 | Manifest offset, exactly 64 |
| 24 | 8 | Manifest length, exactly 96 plus intent length |
| 32 | 8 | Edition offset, exactly manifest offset plus manifest length |
| 40 | 8 | Edition length, from 1 through 65,536 bytes |
| 48 | 8 | Total length, exactly edition offset plus edition length |
| 56 | 8 | Reserved, all zero |

The manifest begins with this exact 96-byte header:

| Offset | Bytes | Meaning |
| ---: | ---: | --- |
| 0 | 8 | Magic `ANSMAN\r\n` |
| 8 | 2 | Manifest version, exactly 1 |
| 10 | 2 | Header size, exactly 96 |
| 12 | 4 | Derivation profile, exactly 1 and equal to the envelope |
| 16 | 8 | Derived workload authority surface |
| 24 | 8 | Edition length, equal to the envelope |
| 32 | 32 | SHA-256 of the complete exact edition bytes |
| 64 | 4 | Plain UTF-8 intent length, from 1 through 512 bytes |
| 68 | 4 | Flags, exactly zero |
| 72 | 24 | Reserved, all zero |

The intent bytes immediately follow that header; the edition immediately follows
the intent. Require checked arithmetic and the complete declared lengths before
slicing. Reject gaps, overlap, reordered sections, unknown versions or profiles,
unknown authority bits, nonzero reserved bytes, padding, and trailing bytes.
There is only one representation of an admitted package. The maximum envelope
extent is 66,208 bytes; an individual artifact must also satisfy the derivation
profile's stricter structural bounds.

The digest covers the edition alone, including its original whitespace and
annotations; it excludes the manifest and envelope, avoiding a recursive hash.
Intent and authority are separately rederived and compared during admission.
Use the pinned RustCrypto `sha2` version `0.10.9` with default features disabled
and `force-soft` selected; the [upstream feature listing](https://docs.rs/crate/sha2/0.10.9/features)
documents those switches. The dependency belongs only to this userspace crate
and its host tooling in this slice. No hand-written hash or FNV substitution is
used for artifact identity. This selection establishes no native CPU-state or
performance qualification.

The envelope is unsigned. SHA-256 binds content identity; it does not authenticate
a publisher, attest compiler provenance, establish trust roots, or prove benign
behavior. Signing and provenance policy remain separate work.

### Derive from a complete, fail-closed NIL profile

Profile 1 accepts a deliberately bounded textual NIL subset defined against
the inspected language forms. It parses and consumes the complete artifact, including
declarations, constants, instructions and relevant comments/annotations. It
does not search for service names or trust an author-declared mask. Budgets
bound source bytes, names, constants, functions and instructions before work
or storage grows. The exact limits are:

| Resource | Limit |
| --- | ---: |
| Edition bytes | 1–65,536 |
| Nonempty plain UTF-8 intent bytes | 1–512 |
| Functions | 64 |
| String constants | 128 |
| Instructions across all function bodies | 4,096 |
| ASCII identifier bytes | 128 |
| Locals per function | 128 |
| Abstract stack values per function | 256 |

The initial function profile has zero arguments and an `i32` or `i64` return.
Straight-line integer operations undergo abstract value-kind and stack-shape
validation; this is not evaluation or a promise that arithmetic cannot fault.
Only explicitly supported instructions and exact static builtin calls are
accepted. The console call forms are `println`, `core__print`,
`console__writeln`, `console__write` and `console__sys_write`, each with one
argument; `CONSOLE_WRITE` and `CONSOLE_WRITELN`; and `INTEROP_CALL intrinsic`
for `console.write` or `console.writeln`, one argument and a `void` result.
Branches, direct calls to user functions, dynamic/function-value calls,
unsupported interop, assembly, types and other unsupported records are refused.
All accepted function bodies contribute to the authority union, including
uncalled bodies. This over-approximation avoids an unproved reachability
analysis. Strings and ordinary comments cannot masquerade as instructions,
while special annotation records cannot disappear as ordinary comments.

Require exactly one supported `@ai_context("...")` annotation attached to
`main`. Its authored text is nonempty plain UTF-8, at most 512 bytes, with no interior
quote, backslash or control character. Preserve its bytes exactly. Escaped or
encoded annotation records, composite/named arguments and contexts outside
this initial shape are unsupported and cause refusal. This avoids decoding a
compiler-retained string a second time and silently changing its meaning.
Missing intent is not replaced with a filename, inferred description or generated
claim. Intent is untrusted descriptive data, not an instruction to the loader or
an authorization decision.

The initial workload vocabulary contains only `console_write = 1 << 0`.
Supported pure computations derive zero; supported console effects derive that
bit. Unknown effects are refused, never treated as pure. The profile does not
claim support for general NIL, binary NIL, the whole runtime intrinsic registry
or the `matrix.execution` corpus. Expanding accepted semantics or authority
mapping requires an explicit derivation-profile revision and corresponding
compatibility evidence.

### Recompute before applying an explicit ceiling

Admission first validates canonical structure and the bounded edition, then
recomputes SHA-256, intent and the complete authority union. Every manifest
field must equal the corresponding derived value. A mismatch refuses the
package; matching self-declared metadata is insufficient.

The trusted embedding supplies an explicit workload ceiling whose only known
bit in this profile is `console_write`. Reject unknown ceiling bits and any
derived authority outside that ceiling before returning an admitted witness.
The effective surface is the intersection. A pure edition admitted under a
console-capable ceiling still has zero effective workload authority. There is
no default all-services ceiling and no implicit permission from an artifact's
identity or authored intent.

Host packaging reports `host_package_validated`, with derived intent, surface
and hashes but no ceiling or granted-services fields. Its internal consistency
check supplies no caller policy. Inspection requires an explicit `none` or
`console_write` ceiling and reports `host_policy_admitted`, including the ceiling
and effective services. Both receipts report `executed: false`.

Keep this semantic workload surface separate from the current ANPG `file_read`
bit. Under ADR 0101 that bit only permits the loader to fetch its one retained
edition; it is neither a general file service nor proof that NIL file operations
are available. A pure workload still needs bootstrap delivery. Do not remove
that delivery mechanism by intersecting the workload mask with the ANPG mask,
or mislabel an entire file intrinsic family as read-only authority.

### Bound host input and distinguish publication from its receipt

The tooling byte-bounds a held input snapshot. Host paths and the output parent
belong to a trusted namespace with cooperating writers; this is not an I/O
deadline or a hostile-filesystem guarantee. Publish a complete, synced temporary
inode by a no-replace hard link, then remove the temporary name. Existing output
must remain unchanged. There is no directory-fsync or power-loss durability claim.

Validation refusal publishes nothing. After publication commits, a cleanup or
receipt-write error must report that the complete output was published and
retained; it must not claim rollback. The measured `/dev/full` receipt failure
returns an error while retaining the exact complete package.

## Compiler boundary and required qualification

The installed compiler's ordinary mode emits a
`__nilaix_register_ai_context_hash` helper returning `void` and
`INTEROP_CALL nilaix register_ai_context_internal 5 void` alongside authored
context. Its CLI exposes no supported mode that omits that helper. These
complete outputs are unsupported by this profile and remain negative admission
fixtures. Do not strip or rewrite the helper, or classify its external operation
as authority-free merely because its apparent purpose is metadata. A production
compiler-to-admitted-edition bridge remains a separate gate.

Positive arithmetic and console fixtures are explicitly hand-authored textual
NIL, with authored intent and independently specified expected surfaces. The
SDK's read-only context inspector checks their structural/context compatibility;
it does not execute them or prove full semantic compatibility. Retain those
inputs separately from complete, unmodified compiler outputs and their exact
source/compiler provenance. Packaging and inspection must agree through the
shared derivation implementation and produce reproducible package bytes.

Tests must cover complete input consumption, malformed and unsupported syntax,
misplaced/duplicate/missing intent, fake instructions in strings/comments,
unknown/direct/dynamic effect paths, all structural budgets, canonical lengths
and reserved bytes, SHA-256 known answers, changed edition bytes, changed intent,
understated and overstated masks, unknown ceiling bits and denied authority.
Rejecting a package must return no admitted witness or partially published
package. A wider ceiling must not add authority absent from the artifact.

Run the applicable host/freestanding checks and repository-required regressions.
The [Program Manifest Admission Log](../../notebook/Development/2026-11-11%20Program%20Manifest%20Admission%20Log.md)
must record exact evidence and failed attempts before qualification is claimed.

## Accepted qualification

The final 25-job matrix passed on its first attempts: seven focused host checks,
the 16 required targets, native-program smoke and Linux-loader-entry smoke.
The verifier confirmed all 608 frozen non-Markdown source files unchanged.
Host suites passed 644 kernel default, 727 kernel all-features, 411 tooling,
35 native-runtime and 18 manifest tests; the focused tooling suite passed 9.
The manifest proof covered four positive fixtures, two complete compiler-output
refusals and 1,659 rejected mutations. Twenty recorded CLI cases matched an
independent Python wire encoder, including explicit ceilings and post-publication
receipt failure.

The native TCG regression at 96/320 MiB reproduced the exact prior qualified
image; Linux remained at request 12 after 11 caller returns. No new KVM or
VirtualBox run was made. The linked development log records package and source
identities, final verification time and the retained preliminary `E0530` tooling
compile failure. This evidence accepts the host prerequisite only.

## Limits and next integration

This slice validates packages on the host. It does not change kernel admission,
native grants or source lifecycle, execute an edition, qualify a NIL interpreter,
qualify source-to-admitted-edition compilation, or advance Linux compatibility.
ADR 0093's native loader-enforcement obligation
remains open after this host prerequisite.

A subsequent native loader must use the same package parser, derivation profile
and admission witness over bytes delivered from the retained source. Its trusted
bootstrap services and the workload's effective services remain distinct. Measure
the real loader's code, memory and stack requirements before choosing an explicit
loader profile; do not silently widen the qualified two-page delivery fixture.
Interpreter execution still requires the unmodified acceptance corpus and its
own proof that every intrinsic effect uses the admitted workload authority.
Qualify the future Rust dispatch implementation directly. The hosted C++ interop
path also invokes FFI hot-reload machinery; its recognition of a console name
does not establish a console-only execution boundary for this profile.
