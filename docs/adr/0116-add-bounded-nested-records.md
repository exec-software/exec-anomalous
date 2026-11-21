# ADR 0116: Add bounded nested records

- Status: Accepted — experimental host profile
- Date: 2026-11-20
- Builds on: ADRs 0102 and 0105–0115
- Scope: Host-only profile 12 for nested nominal record fields and narrowly concrete generic identities.

## Context

[ADR 0115](0115-add-bounded-typed-scalars.md) preserves distinct scalar kinds
and exact declared helper signatures. The complete retained
`aggregates-twin.ny` edition additionally requires record fields holding other
records, a concrete `envelope<i64>` record, and a concrete `maybe<i64>` enum.
Its enum instructions carry generic-erased tags plus explicit concrete type
metadata. These requirements occur in a complete 7,534-byte edition; deleting
declarations or rewriting names would not qualify that artifact.

The pinned VM shares nested record storage through its `StructValue` wrappers.
An absent ordinary field returns integer zero even when its declared type is a
record. A typed candidate cannot interpret that integer as a valid record
handle. Initialization, aliases, cycles and exact nominal identity therefore
need explicit rules before extending admission.

## Decision

### Select profile 12 and preserve existing boundaries

Add **12 in both existing envelope headers** through `nested_records::encode`
and `nested_records::admit`. Inherit profile 11, including typed scalars and
read-only source-debug metadata. Preserve all earlier profile entry points,
their grammar and tag refusals. Keep version 1 framing, canonical extents,
reserved zeroes, exact edition hashing, immutable witnesses and complete
validation before publishing output. Never retry another profile after refusal.

Add `conformance::validate_nested_records` under the existing host-only feature.
The raw witness remains separate from production admission. Only absence of
authored main intent is relaxed; present annotations, complete syntax and
references, independent function CFG checks, authority ceilings and all fixed
resource limits remain required.

### Resolve exact nested field kinds without traversing a graph

Permit record fields of exactly `i64` or an exact declared nominal record type.
Do not add boolean, byte, float, text, array, enum, callable, pointer or optional
record fields. A record field may name a later declaration, its own declaration
or a declaration that refers back to it. Forward, self and mutual references
are permitted because fields contain handles rather than inline record bodies.

Retain each field's borrowed type spelling in fixed scratch storage while
parsing declarations. Resolve all record field types before parsing the first
function. Each non-i64 type must match one declared record exactly. Resolution
uses bounded table lookup, not recursive dependency traversal, instantiation or
topological sorting. Unused declarations must also resolve. No field name may
be duplicated within a layout, and record and enum identities cannot collide.

Store fixed field kinds alongside the existing global field-name IDs in each
record layout. Expose immutable `field_kinds()` in declaration order and
`field_kind(global_field_id)` for lookup within that exact layout. The latter
returns no kind for an ID absent from the receiver's layout. A shared field
name can have different kinds in different layouts; the global name ID alone
never determines its type. Reset scratch state and initialize each new layout
fully when reusing a validation workspace.

`GET_FIELD` consumes a record and pushes the exact declared field kind.
`SET_FIELD` consumes a value of exactly that kind and retains the owner record
on the operand stack, preserving the existing assignment behavior. Structural
similarity does not permit a different nominal record. Helper parameters,
results, local reassignment and CFG joins retain exact record identity. Existing
signature type codes and declaration-order nominal IDs remain unchanged.

### Preserve shared handles and make missing children explicit

Extend each existing execution-owned record cell to hold an i64, a record
handle or an uninitialized marker. New i64 fields retain the existing zero
default. New record fields begin uninitialized; allocation does not recursively
allocate children. Reading an uninitialized record field returns the structured
runtime error `record_uninitialized`. Admission checks types and does not
attempt to prove every field initialized along every alias or execution path.

Writing a nested field stores the existing child handle after checking its
descriptor and exact declared layout. Reading it returns that same handle.
Copies, local storage, direct or indirect calls and returns preserve aliases.
Mutating a child through one alias is visible through every other alias.
Replacing a parent field changes that edge and does not invalidate previously
retained aliases to the former child.

Self-links and cycles are allowed. Validate only the directly addressed owner
and child handles; do not recursively validate, clone, trace or reclaim the
reachable graph. The fixed arena retains allocated descriptors and cells for
the whole execution, including detached objects. Existing object and cell
budgets still bound repeated allocation and cycles. Every public execution
return or failure clears the complete fixed arena, operands and frames without
recursive destruction. Previously acknowledged console effects remain committed
when a later nested-field read fails.

### Admit a narrow concrete generic identity grammar

In addition to existing qualified nominal names, admit one terminal suffix:

```text
QUALIFIED_BASE<ARG>
```

`QUALIFIED_BASE` follows the existing ASCII identifier segments separated by
`::`. `ARG` is exactly one of `i32`, `i64`, `bool`, `u8`, `f64` or `text`.
No whitespace, comma-separated argument list, nested generic argument, nominal
argument, unresolved type parameter, alias, alternate capitalization or suffix
after `>` is accepted. Primitive whole base names remain reserved, including
when a generic suffix is present. Generic enum bases retain the case-insensitive
terminal `Option` and `Result` exclusions.

The whole unqualified bases `ptr`, `ref`, `owned`, `borrow`, `fn` and `tuple`
are additionally reserved case-insensitively when suffixed. The reference
loader and VM give ptr/ref forms special treatment. This profile explicitly
reserves the broader six-name set to keep pointer, ownership, function and
tuple wrapper spellings outside the concrete nominal grammar.
Reject them in unused declarations and field type spellings too. Plain names
such as `ref` and qualified bases such as `app::ref<i64>` retain their nominal
interpretation; this restriction does not alter earlier profile grammars.

Each complete spelling is a distinct concrete identity. Do not instantiate a
template, substitute a field type, infer a generic argument or normalize a
name. For example, a declaration named `box<text>` may still contain only the
explicitly admitted i64 or nested-record fields; its argument is part of nominal
identity and does not widen field storage. `box<i64>` and `box<u8>` cannot merge
as values or function signatures. Generic spellings are admitted only where a
nominal type is expected, not as function, local, global or field names.

Each full nominal identity remains bounded to 128 bytes. Each actual enum tag
also remains bounded to 128 bytes. For a generic enum, apply the tag bound to
its erased base plus `::` plus the variant name, not an invented tag containing
the concrete suffix.

### Resolve generic enum tags using concrete metadata

Keep the existing exact operand counts and unit-or-i64 enum payload policy.
For a generic declaration such as `app::choice<i64>`, instructions use a tag
such as `app::choice::Value`, with the full `app::choice<i64>` identity in the
explicit type operand. That concrete type operand selects the layout. The
tag's erased base must match that layout's base, the variant must exist in that
layout and the payload metadata must match its declaration. A generic suffix
inside the tag itself is unsupported.

Different concrete enum identities may share the same erased base and variant
spellings. `IS_VARIANT` and `UNWRAP_VARIANT` nevertheless require the receiver's
exact concrete layout to match the explicit metadata. A value of `choice<i64>`
cannot be tested or extracted as `choice<u8>`. Existing non-generic enum
behavior and all earlier profiles remain unchanged.

### Retain fixed capacities and structured refusals

Keep 64 record layouts, 32 fields per layout, 256 declared fields total, 64 enum
layouts, 32 variants per layout and 256 variants total. Runtime storage remains
128 record objects and 1,024 record cells per execution. Typed cells increase
the fixed byte footprint relative to the previous i64-only cells; descriptor
and cell counts remain unchanged. Existing 65,536-byte
editions, 4,096 instructions, function, local, stack, frame and transfer limits
remain in force. The core stays `no_std`, allocation-free and unsafe-free.

Unknown or unsupported field types produce the existing unsupported error.
Duplicate identities or fields, mismatched field value kinds, generic tag/base
metadata disagreement and payload disagreement produce syntax errors. Unknown
variants remain unsupported. Malformed generic suffixes or unsupported argument
forms are unsupported; invalid qualified bases retain the existing identifier
syntax error. A stray `>` without `<` remains an invalid identifier. Capacity
violations retain the limit error; an overlong variant tag is checked before
rejecting a generic suffix within it. No failed admission publishes a partial plan
or partially encoded image.

Source metadata remains descriptive and cannot change field kinds, initialize
children, satisfy main intent or grant services. Whole-file syntax and reference
checks still cover dead operations; every function body is checked even when
unused. Dead console operations continue to require the console effect ceiling.

### Distinguish reference semantics from candidate policy

The reference remains compiler/VM revision
`4dad0512dfd3fa4e351f2816196cb48956f5442e`. Its nested `StructValue` copies share
`StructData`, giving the alias behavior retained here. Its ordinary missing-field
read returns integer zero. The candidate's `record_uninitialized` failure is an
explicit finite policy contrast, not a matching reference fault or a null
handle convention.

The reference's generic type canonicalizer supports a broader grammar and
normalization. It preserves all six admitted primitive argument spellings.
The candidate admits only the exact bounded suffix above. The reference's
`IS_VARIANT` compares canonicalized tags without requiring concrete type
metadata equality. Candidate cross-instantiation refusal is a separate static
policy contrast. Construction metadata, candidate signatures and admitted
extraction keep exact concrete identity.

## Evidence contract

Preserve the twenty-three earlier proof/oracle sources and rerun all previous
suites unchanged. Add independent reference and candidate observations that
show exact declared field kinds and concrete identities, including unused
schemas. Reference execution must inspect actual shared record identities and
values without computing field operations in the oracle. Bound its object and
cell ledgers and use nonrecursive observation for cyclic graphs.

Exercise forward, self and mutual dependencies, missing children, integer zero
defaults, nested mutation through aliases, field replacement, cycles, direct
and indirect helper transport, exact generic signatures, different field types
under a shared name, maximum global field IDs, declaration and runtime limits,
cleanup, metadata preservation, authority and profile isolation. Keep finite
policy contrasts, ordinary semantic matches, validation refusals and runtime
faults separate. Guard exhaustion never establishes equivalence.

Retain the complete invocation 7 edition, SHA-256
`70021ba440a8fe8c050607325a6b6d250bbfa5e3e19077bf6cee86b43eb7bef7`,
without rewriting names, trimming helper bodies or adding intent. Preserve all
17 existing raw matches and require this complete root before claiming an
eighteenth invocation and twelfth root. Invocation 35 additionally requires
hygienic locals and remains outside this slice. Invocation 8 additionally needs
mutable indexed storage and allocation; binary NIL v5 and oversized textual
editions retain separate format and capacity gates.

The complete repository checks, independent retained-evidence audits, native
artifact continuity and pinned Linux-loader regression pass: all 56 fresh jobs
on 742 unchanged source files. Thirteen language suites pass 1,377 boundary
cases; all 478 oracle checks pass. The new proof establishes 18 of 45 raw corpus
matches and 12 of 38 roots while all 45 production attempts refuse. The
[Nested Records Log](../../notebook/Development/2026-11-20%20Nested%20Records%20Log.md)
records final hashes, counts and retained evidence. Full corpus qualification
and native NIL execution remain separate closed gates. This host profile
changes no kernel, native admission, desktop or Linux guest ABI contract.
