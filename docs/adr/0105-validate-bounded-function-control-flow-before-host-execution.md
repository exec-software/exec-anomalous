# ADR 0105: Validate bounded function control flow before host execution

- Status: Accepted — bounded host checkpoint; full corpus and native execution remain gated
- Date: 2026-11-11
- Builds on: ADRs 0092, 0093, 0102, 0103 and 0104
- Scope: Separate host-only profile-2 admission and execution of bounded functions and control flow.

## Context

[ADR 0104](0104-develop-a-bounded-host-executor-behind-the-corpus-gate.md)
qualifies experimental host execution of profile 1's straight-line integer and
console subset. Function calls, branches and loops require more than another
dispatch case: admission must establish signatures, resolved targets, operand
stack joins and definite local initialization before the interpreter reaches
them. Authority must still include effects outside the observed execution path.

Keep that work separate from the accepted profile-1 parser, wire acceptance and
native admission loader. The full interpreter corpus gate remains unchanged.

## Decision

### Select a separate profile and witness

Add the host-only, `no_std`, unsafe-free
[`anomalous-program-flow`](../../crates/anomalous-program-flow/src/lib.rs) crate.
It owns profile-2 parsing, canonical encoding, symbol resolution, CFG validation
and a private `admitted_program` witness. It reuses existing content hashing,
error vocabulary and common bounds without changing profile-1 admission.

Keep envelope version 1 and the exact 64-byte `ANPROG` / 96-byte `ANSMAN` header
layout from [ADR 0102](0102-admit-bounded-program-semantic-manifests.md). Require
profile **2 in both headers**. Existing profile-1 admission rejects profile 2;
the new admission path rejects profile 1. Do not reinterpret a package according
to whichever parser happens to accept it. Preserve canonical contiguous extents,
reserved zeros, exact lengths, SHA-256 edition identity, authored intent and
artifact-derived console authority. The unsigned envelope authenticates no
publisher.

The caller supplies a fixed validation workspace. Encoding validates completely
before changing output bytes. Admission checks the complete held package and
trusted ceiling before exposing a witness borrowing both the immutable edition
and validated plan. Plan arrays, resolved instructions and signatures are
read-only through that witness. Public operation descriptions cannot construct
an admitted program or replace its plan.

### Parse and resolve the whole artifact

Retain the bounded integer/text language and console forms, adding direct named
calls, function parameters, labels, unconditional jumps, integer conditional
branches, and ordinary returns. `main` takes zero arguments and returns `i32` or
`i64`; helpers may take integer/text parameters and return integer, text or
`void`. Names remain bounded simple ASCII identifiers. Explicit arithmetic widths
and casts retain their reference semantics. This profile does not admit dollar
temporaries, hexadecimal literals, untyped arithmetic, text concatenation,
dynamic function values, globals, foreign calls or ambient services.

Resolve all function, constant and branch references before CFG analysis.
Require exact call arity. Function names must be unique; labels must be globally
unique and disjoint from function names. Every jump target must belong to its
owning function. Refuse local or parameter names colliding with any function or
console builtin: the reference runtime can otherwise redirect a call through a
text local. Static calls must remain static under the admitted interpretation.

Parse and resolve instructions after a return or jump as well. They contribute
to the authority union, as do unused functions. Dead unknown opcodes, missing
references and malformed calls still cause refusal. The compiler's exact
`RET $ny.generated.nonvoid-fallthrough` marker is allowed only in nonvoid
functions and remains an explicit runtime fault if reached. Retain dead jumps,
labels and markers in complete compiler output; do not strip them to make the
artifact pass.

### Validate reachable flow from every function entry

Validate every function independently from its declared entry, including
functions never called by `main`. Seed its abstract operand stack with parameter
kinds in declaration order. Named locals start uninitialized: the compiler's
reversed `STORE_LOCAL` prologue consumes arguments and binds them. Neither
admission nor execution prepopulates those local slots.

Use a bounded worklist over basic blocks. Incoming operand stacks must have
identical height and kinds at each join. A local is definitely initialized only
when initialized on every incoming path. Intersect that information; an unknown
or uninitialized incoming local removes a previously known initialized kind.
Conflicting kinds on initialized incoming paths cause refusal. A reachable
`LOAD_LOCAL` requires definite initialization, and subsequent stores are checked
against the known kind at that point. Do not treat a value assigned only inside
a loop as initialized when the loop takes zero iterations.

Branch conditions must be integer values. A loop backedge must satisfy the same
join invariants as a forward edge. Calls consume the exact parameter kinds and
produce the declared result kind; signatures allow recursive calls to be
validated without recursively traversing execution. Ordinary `RET` requires
exactly its declared result count and kind. Reachable control cannot fall out of
a function. The generated-fallthrough marker is a terminal fault operation,
not an ordinary successful return.

Entry-unreachable instructions receive complete syntax, reference and authority
checks, but **no invented stack or local state**. Their operand correctness is
not claimed. All branch targets are fixed before analysis; any target reachable
from an entry joins the validated worklist. This distinction preserves complete
compiler artifacts without permitting dead text to conceal authority.

### Bound validation and iterative execution separately

| Resource | Bound |
| --- | ---: |
| Edition / plain intent bytes | 65,536 / 512 |
| Functions / constants | 64 / 128 |
| Instructions in the artifact | 4,096 |
| Parameters per function | 8 |
| Locals per function or runtime frame | 128 |
| Basic blocks across the artifact | 256 |
| Validation instruction transfers | 1,048,576 |
| Runtime operand values across all frames | 256 |
| Runtime call frames, including `main` | 16 |
| Runtime dispatched instructions | 65,536 |
| One console payload / acknowledged output total | 4,096 / 65,536 bytes |

The separate
[`flow` executor](../../crates/anomalous-program-executor/src/flow.rs) uses a
caller-owned execution workspace and iterative dispatch. Calls retain a stack
floor, return position and isolated locals. Arguments remain in declaration
order on the operand stack; callees cannot pop the caller's preserved prefix.
Returns leave only the declared result above that floor. Do not insert implicit
casts at call or return boundaries. Text values borrow checked literal storage.

Recursion consumes explicit frames, not the Rust call stack. Every dispatched
instruction, including labels and loop iterations, consumes runtime fuel.
Callers may lower runtime limits but cannot raise their hard bounds. Limit
exhaustion and generated fallthrough are explicit errors. Clear internal frame
and value storage at each execution boundary; a failed machine is not resumable.

Preserve ADR 0104's console contract: validate the complete nonempty UTF-8 payload
before the sink call, allow only LF/TAB controls, perform empty non-newline writes
as no-ops, and never silently split or truncate output. Record attempted calls
and acknowledged bytes separately. A refusing sink can already have effects;
there is no automatic retry or rollback of earlier output.

The cores allocate no heap storage and use no unsafe code. Host tests and probes
may use `Box` and explicit 8 MiB test-thread stacks for their large fixed metadata
workspaces. Those placements establish no native stack or mapping geometry.

## Acceptance contract

Retain six complete outputs from compiler commit
`4dad0512dfd3fa4e351f2816196cb48956f5442e`, produced using the explicit
registration-suppression option from ADR 0103. Their required results are:

| Fixture | Integer result | Console bytes |
| --- | ---: | --- |
| `branches` | 32 | Empty |
| `calls` | 42 | Empty |
| `early_returns` | 0 | Empty |
| `recursion` | 120 | Empty |
| `text_void` | 0 | `yes` followed by LF |
| `while` | 13 | Empty |

Require independent exact semantic comparisons, hostile CFG and symbol tests,
cross-profile wire refusals, fixed-storage boundaries, frame/fuel exhaustion,
caller-prefix isolation and truthful service-failure accounting. Preserve
profile-1 regressions and verify the native admission artifact remains identical.
Retain exact inputs, source/tool identities, failed attempts and all required
and affected checks.

The retained profile-2 host proof passes **77 cases**: 38 exact semantic matches,
two matching runtime faults, four deliberate runtime-policy refusals against
completed finite reference runs, four candidate-only bounded-policy cases, and
29 expected admission refusals. Infinite branch/call cycles and invalid caller
limits run only in the bounded candidate; a reference timeout is never a match.
The six complete compiler fixtures above are included in the 38 semantic matches.
Eighteen additional checks of those same editions confirm that profile 1 refuses
their extended raw NIL syntax; separate canonical-envelope unit tests check
profile-tag isolation in both directions.

The unchanged profile-1 proof also passes all **126 cases**, retaining its
100 semantic matches, 20 matching runtime faults and six deliberate output-policy
refusals. Independent retained-evidence verifiers check both proofs, their raw
process outputs and exact input/tool identities. Each proof separately runs the
unmodified upstream runner: 97 compiler calls, 38 root fixtures and 45 interpreter
invocations. The reference passes, while **all 45 candidate invocations refuse
admission** in each ledger. This evidence keeps `candidate_qualified: false` and
`native_execution: false`.

All 25 final repository qualification jobs passed on 651 unchanged non-Markdown
source files, including the required kernel/desktop regressions and fresh TCG
native admission, delivery and Linux loader regressions. Native admission
artifacts are byte-identical to the preceding checkpoint. The complete receipt,
independent verification and retained failed preliminary attempts are in the
[Program Flow Executor Log](../../notebook/Development/2026-11-11%20Program%20Flow%20Executor%20Log.md).

## Remaining gates

The unmodified corpus still requires 38 root fixtures, 45 interpreter invocations,
three schema comparisons and its compiler/diagnostic obligations. Bounded
profile-2 progress does not waive any of them or count an admission refusal as
compatibility. Native execution remains gated; this slice changes no kernel
feature, native profile, startup descriptor, service gate or diagnostic report.
Task-bound effect mediation, wider language and service support, and a measured
native interpreter artifact require further qualification.
