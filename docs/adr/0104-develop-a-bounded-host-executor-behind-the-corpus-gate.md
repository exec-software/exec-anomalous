# ADR 0104: Develop a bounded host executor behind the corpus gate

- Status: Accepted for experimental host-only development
- Date: 2026-11-11
- Builds on: ADRs 0092, 0093, 0102 and 0103
- Scope: Experimental host-only execution of admitted profile-1 editions.

## Context

[ADR 0103](0103-admit-compiler-produced-manifests-in-a-native-loader.md)
qualifies native delivery and semantic admission of complete compiler outputs.
The loader executes no NIL instructions. The next prerequisite is a bounded
execution core whose observable semantics can be compared with the reference
runtime before introducing native workload execution.

[ADR 0092](0092-run-nil-on-anomalous-with-a-native-interpreter.md) selects the
interpreter route. Its
[Runtime Owner Reply](../../notebook/Development/2026-10-15%20NIL%20Runtime%20into%20Anomalous%20%E2%80%94%20Runtime%20Owner%20Reply.md)
selects Rust and explicitly gates the second VM on the existing, unmodified
`matrix.execution` corpus. Profile 1 is much smaller than that corpus. A few
arithmetic and console demonstrations cannot satisfy or replace the gate.

## Decision

### Keep the candidate host-only

Implement an experimental `no_std`, unsafe-free executor with fixed storage and
no heap allocation. Exercise it through host tests and differential proof
tooling. Do not link it into a kernel image or native vessel. This slice adds no
kernel feature, mapping profile, native gate operation, startup field or native
report revision. The accepted native admission proof retains its nonexecution
contract.

Successful bounded host evidence qualifies only the candidate's explicitly
tested profile. It does not qualify a general NIL interpreter, a native
application runtime, or the task-bound mediation of interpreted effects.

### Execute only a held admission witness

Require the private `admitted_edition` witness from the shared manifest crate.
Keep the exact admitted bytes immutably borrowed through execution. Identity,
authored intent and complete artifact-derived authority must have passed
admission against an explicit trusted ceiling before dispatch begins.

Retain the existing profile-1 language and structural bounds from
[ADR 0102](0102-admit-bounded-program-semantic-manifests.md). Execute `main`;
unused accepted functions continue to contribute to admission's authority
union. Unsupported records or instructions cannot become executable by being
ignored. Share validated instruction decoding with admission rather than
introducing a permissive alternate parser.

| Execution resource | Bound |
| --- | ---: |
| Value stack | 256 values |
| Locals | 128 |
| String constants | 128 |
| Instruction steps | At most 4,096; default 4,096 |
| One published console payload | 4,096 bytes |
| Total published console bytes | 65,536 bytes |

These bounds supplement the complete edition's existing limits. Resource
exhaustion is an explicit execution error. It must not wrap counters, silently
truncate values or output, publish partially validated output, or expand storage.

### Preserve reference semantics and explicit effects

Define accepted instruction behavior against the pinned reference compiler and
runtime at commit `4dad0512dfd3fa4e351f2816196cb48956f5442e`. Typed integer
arithmetic narrows operands to its declared width and wraps addition,
subtraction, multiplication and negation. Division and modulo reject zero and
the signed minimum divided by minus one. Shifts validate the original count
before narrowing; signed right shift is arithmetic. Comparisons preserve exact
integer precision. Casts, locals, stack operations, string decoding and console
newline behavior require differential evidence rather than inference from
opcode names.

All output crosses a supplied, explicit service interface. Require the
witness's console authority before a console instruction can publish. The
loader's edition-read grant is not available to the workload. An unknown or
unsupported service remains refused; host FFI resolution, dynamic loading,
hot reload, filesystem access and ambient host services are not inherited.

Check operands, arithmetic errors, authority, step allowance and output budgets
without exposing a partially validated output request. A nonempty console
payload must be UTF-8 and contain no controls except LF and TAB. Format and
validate the complete payload, including any newline, before calling the sink.
An empty write without a newline is a no-op, with no service
publication. A newline-producing instruction still emits its newline when its
text value is empty. Do not sanitize, split or truncate an inadmissible payload
to make it pass.

The synchronous sink acknowledges a complete request or returns refusal. A
refusing sink may already have internal or external effects; the executor cannot
infer atomic I/O or undo those effects. Record attempted calls separately from
acknowledged output bytes, stop on refusal and never retry automatically.
Policy or limit rejection before the sink call publishes nothing from that
instruction. The execution result must expose arithmetic, limit, authority,
text-policy and service failures explicitly. Private stack state may change
before an error; the failed machine is not resumable.

This is not a transaction over the whole program. Successfully published earlier
output remains observable when a later arithmetic operation, budget check or
service call fails. Report committed progress and the failure accurately;
do not imply rollback, erase earlier output from evidence, or substitute a
successful return value after a refusal.

### Preserve the full corpus gate

Pin the reference tools, corpus files and runner. At the selected compiler
revision the unmodified `tests/matrix/run_execution_tests.py` selects **38 root
fixtures**, including Ny twins and a short `.mat` source after excluding
secondary pages. Its auxiliary execution checks bring the required interpreter
invocations to **45**. Three twin pairs require identical emitted struct, enum
and class schemas. Eight fixtures request the standard library; the existing
`telemetry-relay` marker permits exit-only checking for that one clock-dependent
fixture. Preserve that explicit exception rather than broadening it.

The same runner also requires compiler diagnostics, foreign-keyword guidance,
declaration-intent retention, include resolution, source/binary import diagnostic
behavior, mixed-case source suffixes and debug metadata. Record these obligations
alongside execution outcomes. The older handoff's count of 32 is historical;
it does not authorize selecting a smaller suite from the pinned runner.

Maintain a full qualification ledger with each obligation and its actual state.
Distinguish reference-runtime success, candidate success, candidate refusal,
failure and work not yet run. A reference pass does not become a candidate pass.
Rejecting an unsupported positive corpus fixture is safe behavior but does not
satisfy its execution expectation. No fixture editing, helper stripping,
expected-output changes or narrowed selection may convert an incomplete corpus
into qualification.

The current corpus includes calls, branches, recursion, aggregates, classes,
faults, owned buffers and pointers, plus standard-library, file, time, entropy
and parallel operations. These exceed profile 1 and remain explicit work.
Compiler/schema conformance, interpreter semantics, manifest admission and
native service mediation are separate obligations: one green layer cannot
stand in for another.

## Acceptance for this prerequisite

Require exact differential output and outcome evidence for every accepted
instruction and console form, including numeric boundaries, decoded text,
empty output, output limits and partial progress before a later failure.
Compare immutable identical edition bytes under pinned tools. Cover malformed
admission, denied ceilings, fixed-storage limits and side-effect-free policy
rejection before the current instruction reaches the sink. Retain source identities, failed attempts and all
required and affected regression results.

The full qualification ledger must remain visibly incomplete until the
unmodified corpus passes on the candidate. Host proof tooling must label its
bounded scope and must not claim native edition execution. Results for
this prerequisite are recorded in the
[Host Program Executor Log](../../notebook/Development/2026-11-11%20Host%20Program%20Executor%20Log.md).

## Native execution remains gated

After corpus qualification, native integration still requires a measured build
and stack profile, admitted-witness dispatch, task-bound effect mediation,
output and exit-result contracts, fault behavior and complete resource cleanup.
The existing native report means nonexecution, its console counts describe a
fixed loader marker, and the terminal operation currently requires status zero.
Those contracts must not be silently repurposed. Wider language, service or
format support requires its own reviewed qualification.
