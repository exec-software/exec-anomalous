---
title: "Shell Language — Research Questions"
date: 2026-09-08
tags: [research, shell, language, security, privacy, human-machine]
status: active
domain: semantic-control
---

# Shell Language — Research Questions

## Working premise

The shell should be a human-oriented notation over Anomalous objects, rites,
invocations, plans, and results—not a privileged alternate control plane. A
machine should be able to submit the corresponding structured invocation
without generating fragile command text, and a person should be able to inspect
an equivalent, stable rendering.

This premise follows the proposed shared semantic control plane. It is a design
hypothesis until the relevant ADR is accepted and prototypes demonstrate it.

## Non-negotiable evaluation criteria

1. **Explicit effects:** state changes, authority requests, network access, and
   secret disclosure are previewable before execution.
2. **No ambient privilege:** code receives only the sigils granted to its
   invocation and cannot gain authority through string composition.
3. **Unambiguous structure:** parsing and expansion cannot depend on hidden
   global state or execute code as a side effect.
4. **Human legibility:** common tasks are concise, errors point to source and
   explain recovery, and accessibility does not depend on color or layout.
5. **Machine legibility:** syntax maps losslessly to a versioned AST or direct
   invocation schema with stable types and structured errors.
6. **Deterministic planning:** clients can obtain a plan digest for the exact
   effects later authorized and executed.
7. **Safe composition:** typed results preserve provenance and do not become
   executable text merely by flowing through a pipeline.
8. **Bounded execution:** time, memory, output, recursion, concurrency, and
   cancellation have enforceable semantics.
9. **Privacy by construction:** history, logs, traces, completions, and AI
   context have disclosure and retention policy rather than implicit capture.
10. **Debuggability:** source spans, trace identity, intermediate values, and
    failure domains survive from text or structured request to result.

## Syntax and semantic questions

- Is the core language expression-oriented, command-oriented, or a deliberately
  small command notation over a separate data language?
- Which values are native: records, tables, lists, byte streams, paths, handles,
  results, futures, plans, secrets, sigils, and governed context?
- Are variables immutable by default? How are lexical scope and modules made
  obvious in both text and serialized ASTs?
- How do interpolation and quoting avoid the multi-pass expansion traps of
  traditional shells?
- Do pipelines transport typed values, bytes, event streams, or an explicit
  adapter between them?
- Are failure values ordinary typed results, propagation operators, or control
  flow? How does a pipeline retain partial-failure attribution?
- How are concurrency, cancellation, deadlines, retries, idempotency, and
  compensation written without hiding expensive behavior?
- How does a user request `preview`, approve a plan digest, and then execute
  exactly that plan?
- How are foreign programs invoked without confusing their string-oriented
  arguments and exit codes with native rites and structured results?
- Can a machine emit the AST directly while still producing canonical source
  suitable for review, diffs, and audit?
- Which Unicode normalization and confusable-character rules apply to
  identifiers, paths, and capability names?
- What is deliberately impossible in the language?

## Threats to exercise early

| Threat | Minimum test case |
| --- | --- |
| Injection through interpolation | Untrusted text containing separators remains one data value. |
| Command or path confusion | Resolution identifies the exact executable or rite before approval. |
| Ambient environment leakage | A script cannot enumerate undeclared secrets or host context. |
| History and trace leakage | Secret-typed values are redacted and excluded from persistence by policy. |
| Parser differentials | Text, canonical AST, and direct machine invocation agree on one meaning. |
| Unicode deception | Visually confusable authority names are rejected or made unmistakable. |
| Expansion denial of service | Input, nesting, generated values, and output are bounded. |
| Time-of-check/time-of-use | Authorization binds the resolved plan and relevant object versions. |
| Hidden effects in helpers | Functions declare effects transitively; preview cannot omit them. |
| AI context poisoning | Model-provided text is data and cannot grant itself syntax or sigils. |

## Systems and ideas to study

Study behavior and failure modes, not surface punctuation:

- POSIX shells for ubiquity, composition, quoting hazards, and process semantics;
- Nushell and PowerShell for structured pipelines;
- Oil/YSH and Elvish for attempts to regularize shell grammar and values;
- Rust, Swift, and ML-family languages for explicit results and type-driven
  expression design;
- Nix, Dhall, and CUE for declarative or constrained evaluation;
- capability-secure languages and object-capability systems for authority; and
- SQL and dataflow systems for plans, typed transformations, and explainability.

Each comparison should use the same task corpus and record what is adopted,
rejected, or still unknown.

## Initial primary references

These are starting points for behavioral comparisons, not endorsements or a
substitute for running the common task corpus:

| System | Primary material | Property to test |
| --- | --- | --- |
| Nushell | [Pipelines](https://www.nushell.sh/book/pipelines.html), [data types](https://www.nushell.sh/book/types_of_data.html) | Structured values through pipelines and the boundary to byte-oriented programs |
| PowerShell | [Pipelines](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_pipelines), [objects](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_objects) | Object transport, discovery, streaming, and effect visibility |
| Oil/YSH | [Oil versus shell idioms](https://www.oilshell.org/release/0.11.0/doc/idioms.html) | Explicit splitting and globbing versus multi-pass word expansion |
| Elvish | [Pipelines and I/O](https://elv.sh/learn/pipelines-and-io.html) | Coexistence of typed values and byte streams |
| Dhall | [Safety guarantees](https://docs.dhall-lang.org/discussions/Safety-guarantees.html) | Pure evaluation, totality, imports, and analyzability limits |
| fish | [Language tutorial](https://fishshell.com/docs/current/tutorial.html) | Interactive ergonomics and reduced post-expansion splitting |

## Proposed prototype sequence

1. Write a tiny grammar and canonical AST with no executor.
2. Prove parse/print/parse equivalence with golden cases and generated inputs.
3. Add pure evaluation over fixed values with resource limits.
4. Model `help` and `system.info` as typed rites and invoke them through both
   source text and direct structured requests.
5. Add plan, denial, cancellation, structured error, and audit test cases before
   the first mutating operation.
6. Introduce foreign byte-stream commands only through an explicit adapter.

## Initial task corpus

A syntax candidate should express these without special pleading:

- inspect CPU and memory information;
- list vessels and explain why one is unhealthy;
- filter structured events by level and trace;
- preview, authorize, and cancel a package transaction;
- pass untrusted text to a foreign program as one argument;
- start two read-only operations concurrently with a deadline;
- handle one failed item without discarding successful typed results;
- request a secret for one operation without printing or persisting it; and
- render one invocation equivalently for a terminal, accessibility client, and
  machine client.

## Exit condition for research phase zero

Advance to an evaluator implementation only after at least two contrasting
language sketches have been tested against the corpus and threat cases, with a
written explanation of their ambiguity, authority, error, and round-trip
behavior. Novelty alone is not a passing result.
