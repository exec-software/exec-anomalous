# ADR 0114: Add bounded textual source-debug metadata

- Status: Accepted — experimental host profile; full corpus and native execution remain gated
- Date: 2026-11-20
- Builds on: ADRs 0102 and 0105–0113
- Scope: Host-only profile 10 for descriptive textual source paths and lexical bindings.

## Context

[ADR 0113](0113-add-bounded-nominal-unit-and-integer-enums.md) provides the
current bounded nominal value vocabulary. The complete textual source-debug
auxiliary artifact already uses supported record, integer and direct-call
operations. Its 23 reserved `# source_debug` records are the remaining parser
blocker. This artifact is one auxiliary interpreter invocation, not an
additional corpus root.

The pinned compiler emits these records immediately before selected actual
instructions. They describe a source path and lexical bindings at that point;
they do not establish executable local declarations, operand types, authored
intent or authority. Supporting them requires strict parsing and a useful
read-only representation, rather than ignoring arbitrary comment syntax.

## Decision

### Select an explicit profile and preserve existing admission

Add profile **10 in both existing envelope headers** through
`source_debug::encode` and `source_debug::admit`. Inherit profile 9 while
preserving every earlier profile's grammar and tag refusals. Retain version 1,
canonical extents, reserved zeroes, exact edition hashing, immutable validation
witnesses and complete validation before publishing output. Do not retry a
different profile after refusal.

Add `conformance::validate_source_debug` under the existing opt-in host-only
feature. Its raw checked witness remains distinct from production admission.
Only missing authored main intent is relaxed. Present annotations, complete
syntax and references, independent function CFG checks, authority ceilings and
all resource bounds remain required. Metadata cannot satisfy missing intent or
grant a service.

### Parse a strict bounded reserved record

Admit this textual form inside a function:

```text
# source_debug INDEX "PATH" BINDING_COUNT ["NAME" "STORAGE" "TYPE" SCOPE_DEPTH]...
```

The bracketed group repeats exactly `BINDING_COUNT` times; brackets are notation,
not accepted syntax. The marker is case-sensitive. Spaces or tabs may separate
fields and follow `#`; no space after `#` is also permitted. Ordinary surrounding
whitespace and blank lines retain existing handling. Do not accept arbitrary
comments, altered marker spellings or extra operands.

All numeric fields are bare, complete ASCII unsigned decimal tokens, including
leading zeroes. Quoted numbers, signs, hexadecimal spellings and overflow of the
host `usize` parser are syntax errors. `INDEX` must equal the next actual
instruction's zero-based index within its function. Reset that index at each
function entry. Include `LABEL` and all other actual operations in the count;
the metadata record itself consumes no instruction or runtime fuel.

Each record attaches to exactly the next actual instruction. Reject records
outside a function, duplicate pending records, records orphaned by a function
end or end-of-file, and records whose index does not match. Blank lines may
intervene; unsupported comments or declarations do not become instructions and
cannot discard pending metadata. Validate records in unused functions and dead
instruction regions too.

Require double quotes around every path, name, storage and lexical type string.
Each decoded string must be nonempty. Accept literal valid UTF-8, including
spaces and semicolons, and only the two escapes `\\` and `\"`. Preserve exact
decoded bytes without path canonicalization, normalization or case folding.
Known control escapes (`\n`, `\r`, `\t`, `\v`, `\f`, `\0`) are syntax errors;
other escape spellings are unsupported. Reject raw ASCII control bytes and DEL
inside strings. The inherited whole-edition Unicode control check also remains
in force; this includes C1 controls that the general reference metadata helper
would permit. Existing whole-edition invalid UTF-8 refusal remains unchanged.

Commas, unquoted string operands and trailing semicolon comments are outside
this profile's reserved-record grammar. An embedded semicolon inside a quoted
string is ordinary descriptive data. Do not use the instruction comment parser
to split a reserved metadata record.

Bound each decoded source path to **16,384 bytes**, each decoded lexical name,
storage name and type to **128 bytes**, each record to **128 bindings**, and each
scope depth to **1,024**. Names and storage names must each be unique within the
record when compared as decoded bytes. Their namespaces need not be disjoint;
the same name may also appear in a later instruction's record. Types and paths
have no uniqueness requirement. The edition's **65,536-byte** and **4,096 actual
instruction** limits remain unchanged. At most one record can attach to each
instruction, so no separate unbounded metadata table exists. Combinations of
individual maxima remain subject to the edition bound.

Malformed placement, operand structure, indices, empty strings, duplicate names
or storage and controls produce the existing structured syntax error. Decoded
string, binding-count and scope-depth capacity violations produce the existing
limit error. Unknown escapes produce the existing unsupported error. No invalid
edition can publish a production image or an immutable checked plan.

### Preserve immutable metadata without introducing runtime state

Keep one optional borrowed encoded record per actual plan instruction. The
validated plan exposes it through `program.source_debug(global_index)`; the
returned record exposes its per-function instruction index, path, binding count
and bounded binding iterator. Out-of-range or unannotated global indices return
no record. Paths and binding strings provide read-only encoded contents,
decoded-byte iteration and copying into caller-owned storage. Check destination
capacity before copying so a short output remains unchanged.

All views borrow the exact edition held by the existing immutable witness.
Callers cannot construct checked strings or records from arbitrary bytes, and
cannot mutate a plan through an accessor. Validation checks every binding and
duplicate before publication; accessor iteration does not perform filesystem
access or reinterpret the lexical type vocabulary. Clear all stored optional
record references on workspace reset so earlier editions cannot leak through a
reused plan.

The storage uses a fixed array of optional borrowed string slices, with bounded
temporary duplicate-check scratch space. There is no decoded string arena,
allocation or per-record array of 128 permanent bindings. The core remains
`no_std` and unsafe-free. No executor value, frame, operation or cleanup state
changes; metadata adds neither instructions nor fuel charges. Existing physical
edition-line diagnostics remain intact. This checkpoint does not introduce a
debugger protocol, source-file loader, native debug format or privileged API.

### Distinguish reference semantics from profile policy

Compiler/VM revision `4dad0512dfd3fa4e351f2816196cb48956f5442e`
uses generic textual operand parsing for reserved debug records. It accepts
unquoted string operands, quoted numeric operands, optional operand commas and
unknown escape sequences preserved literally. Its helper permits up to 1,024
bindings and 16,384 bytes per string; its textual parser separately requires a
nonempty source path. Its ASCII control check is narrower than the candidate's
inherited whole-edition control check.

The profile's stricter operand spellings, escape vocabulary, lexical limits and
inherited control boundary are explicit finite-reference policy contrasts.
They are not VM faults or language-wide semantic changes. Accepted metadata
must preserve the actual pinned loader's path, index and lexical binding bytes.
No source path may be rewritten to make an edition hash stable. In particular,
the upstream runner embeds a fresh temporary path in its source-debug fixtures;
retain and hash each exact captured edition.

## Evidence contract

Preserve the nineteen earlier proof/oracle source files and rerun all earlier
proof suites unchanged. Add a separate guarded proof using both actual pinned
textual loaders and the actual VM. Independently compare the candidate's
validated metadata with executable and module metadata from the reference,
including per-function indices, decoded UTF-8, escaped quotes/backslashes,
semicolons, lexical names, storage, type strings and scope depths. A matching
runtime exit alone is insufficient evidence that metadata survived parsing.

Exercise accepted maxima and one-over-limit refusals, exact operand counts,
quoted/bare spelling policies, controls and invalid UTF-8, duplicate names and
storage, malformed and orphan placement, instruction indices including labels,
unused helpers, dead records, intent/authority isolation, unchanged execution
counts and workspace reuse. Check read-only access and atomic short-buffer
copying. Keep finite-reference policy contrasts separate from semantic matches,
validation refusals and matching runtime faults; guard or timeout exhaustion
cannot establish equivalence.

Run the unmodified upstream runner and all retained complete artifacts through
separate raw and production lanes. Preserve its 97 compiler calls, 38 roots,
45 interpreter invocations, auxiliary obligations and telemetry exit-only
exception. Keep all fifteen prior raw matches. Require the complete unchanged
textual source-debug auxiliary invocation to match before claiming increased
coverage. This can add one auxiliary invocation and no roots. Do not remove
unused helpers, normalize temporary paths, strip metadata, inject intent or
rewrite operations. The binary NIL v5 source-debug fixture remains a separate
container and section-parser gate.

Require complete repository checks, independent retained-evidence audits,
native artifact continuity and Linux-loader regression before accepting this
checkpoint. Full interpreter qualification and native NIL execution remain
separate closed gates. This host profile changes no kernel, native admission,
desktop, task-service mediation or Linux compatibility boundary.

## Qualification

All 50 fresh final jobs pass on 726 unchanged non-Markdown source files.
The eleven language suites total 1,016 boundary cases; the new source-debug
suite contributes 111. All 260 oracle checks and 13 explicit Linux-input
selection checks pass. Sixteen of 45 complete raw invocations match, including
ten of 38 roots; production refuses all 45. Independent audits verify exact
metadata, full target results, source/tool/input identity, original-checkout
preservation and unchanged native artifact identities.

The initial cohort stopped at the existing Linux input hash guard after a host
package upgrade. It remains retained without final qualification credit. Explicit
host input-path selectors now allow the same reviewed bytes without changing
hashes, guest paths or Linux policy. The entire final cohort was rerun after
that tooling change. The [Source Debug Metadata Log](../../notebook/Development/2026-11-20%20Source%20Debug%20Metadata%20Log.md)
records both cohorts, input provenance, measurements and remaining gates.
