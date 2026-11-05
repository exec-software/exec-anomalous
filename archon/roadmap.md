# Archon roadmap

Archon is the planned Anomalous scanning and security-assessment subsystem:
one tool and one semantic model for scanning code, configuration, permissions,
dependencies, runtime state, and other surfaces for security-relevant issues.
It is written in **Matrix**, the second surface language over the NIL runtime
shipped by the Ny project (see the public NIL framework and [runtime requirements](../docs/optional-runtime.md)), and it compiles and runs
against the project-wide runtime at `runtime/`.

Archon is intended to become a core part of the operating system that people
and agents use to find *any* kind of security issue anywhere — not only a
signature-matching virus scanner. This increment is deliberately standalone and
host-side: it establishes the finding model and a first analyzer before any
privileged or kernel integration.

This file tracks the work as small, reviewable units. Each unit names its
deliverable, its evidence, and its status. Update the status line as units land.

Legend: `TODO` not started, `WIP` in progress, `DONE` landed with evidence,
`BLOCKED` waiting on another unit or an open decision.

---

## Unit 0 — Shared Matrix-capable Anomalous Ny runtime

**Goal.** Archon consumes the one project-wide, Matrix-capable Ny runtime under
`runtime/`, exactly as Oculus does, instead of carrying its own SDK.

**Evidence.** Provided by the Oculus work: `scripts/refresh-ny-runtime.sh`
builds and activates `runtime/`; `oculus/src/hello.mat` compiles under
`runtime/nycompiler` and runs under `runtime/nyinterpreter`.

**Status.** DONE

---

## Unit 1 — Archon hello and project skeleton

**Goal.** A first Matrix program under `archon/` that establishes the project
shape: a book per concern, a Makefile that drives the shared runtime, and
runnable smoke targets.

**Tasks.**

- [x] T1.1 Scaffold `archon/` layout (books, Makefile, README, roadmap).
- [x] T1.2 `archon.hello` — smallest runnable Archon page (`src/hello.mat`).
- [x] T1.3 A Makefile that compiles and runs pages through `runtime/`.
- [x] T1.4 Document the build/run contract in `archon/README.md`.
- [x] T1.5 Wire `make archon-smoke` (and `archon-check`) from the repository
      root, and record `archon/matrix-issues.md`.

**Evidence.** `make -C archon smoke` compiles and runs `src/hello.mat`, prints
the banner, and asserts it.

**Status.** DONE

---

## Unit 2 — Finding semantic model

**Goal.** Model a finding as a first-class, machine-readable object before any
analyzer exists, so all later work shares one vocabulary. The model must be
broad enough to describe issues on any surface (code, configuration,
permissions, dependencies, runtime, data), not just code.

**Tasks.**

- [x] T2.1 Severity — `Info`, `Low`, `Medium`, `High`, `Critical`, with an
      ordinal rank and a stable name (`archon.model.severity`).
- [x] T2.2 Confidence — `Speculative`, `Low`, `Moderate`, `Likely`,
      `Certain`, with a name (`archon.model.confidence`).
- [x] T2.3 Category — the surface under assessment: `Code`, `Security`,
      `Permission`, `Configuration`, `Dependency`, `Runtime`, `Data`
      (`archon.model.category`).
- [x] T2.4 Location — target label, path, line, column, and matched excerpt,
      with a `path:line:column` summary (`archon.model.location`).
- [x] T2.5 Finding — identity, rule id, title, detail, severity, confidence,
      category, location, guidance, with a one-line summary
      (`archon.model.finding`).
- [x] T2.6 Rule — `knowledge-rule`: identity, title, needle, guidance,
      severity, confidence, category (`archon.model.rule`).
- [x] T2.7 Target and report — `scan-target` and the `scan-report` summary
      (`archon.model.target`, `archon.model.report`).
- [x] T2.8 Model smoke — constructs each type and checks its helpers
      (`archon/model/smoke.mat`).

**Evidence.** `make -C archon model-smoke` compiles the model books and asserts
the rendered severity, category and location summary.

**Status.** DONE

---

## Unit 3 — Rule engine and the first text scan

**Goal.** Turn a target and a set of pattern rules into findings: split a text
buffer into lines, match each rule against each line, and summarize the result
into a `scan-report`. This is the smallest end-to-end scan.

**Tasks.**

- [x] T3.1 Text patterns — `split-lines` (implemented in Matrix, because the Ny
      `split_lines` returns a `vec<text>` unusable from Matrix) and
      `line-contains` (`archon.engine.patterns`).
- [x] T3.2 Rule application — `rule-matches` and `finding-from-rule`
      (`archon.engine.rules`).
- [x] T3.3 Scanner — `scan` (target × rules → findings) and `summarize`
      (findings → highest severity) (`archon.engine.scanner`).
- [x] T3.4 Machine-readable report line — `report-summary`
      (`archon.model.report`).
- [x] T3.5 Engine smoke — scan one sample target with three rules across three
      categories and assert the finding count and highest severity
      (`archon/engine/smoke.mat`).

**Evidence.** `make -C archon engine-smoke` prints a machine-readable report
line and asserts `findings=3 highest=critical rules=3`.

**Status.** DONE

---

## Unit 4 — Matrix rule set and the standalone CLI

**Goal.** Make Archon runnable as a real command-line tool and give it the first
language-targeted rules: a Matrix-specific rule set plus a language-agnostic set,
selected by the catalog. This is the first increment that reads a real file from
disk, takes its paths from `argv`, and reports findings a person or agent can
act on.

**Tasks.**

- [x] T4.1 Generic rules — language-agnostic code/security/permission patterns
      (`archon.rules.generic`, `src/archon/rules/generic.mat`).
- [x] T4.2 Matrix rules — Matrix-specific constructs grounded in the Matrix
      guide: inline assembly, runtime intrinsics, manual memory, unbounded
      loops, raw text blocks, broad imports, agent self-description and casts
      (`archon.rules.matrix`, `src/archon/rules/matrix.mat`).
- [x] T4.3 Catalog — `rules-for kind` and `rule-count-for kind` select the rule
      set from the target kind (`archon.rules.catalog`,
      `src/archon/rules/catalog.mat`).
- [x] T4.4 Standalone CLI — `archon.cli.scanner` reads `argv`
      (`ny.lang.system::arg_count`/`arg`) and files (`ny.lang.io::read_text`),
      infers the kind from the extension, scans, and prints findings and the
      machine-readable report; it exits non-zero with no path or an unreadable
      path (`src/archon/cli/scanner-cli.mat`).
- [x] T4.5 Fixture and CLI smoke — `testdata/insecure.mat` is scanned and its
      findings and report line are asserted (`make cli-smoke`).

**Evidence.** `make -C archon cli-smoke` scans `testdata/insecure.mat` and
asserts `target=testdata/insecure.mat findings=6 highest=critical rules=15`,
including a critical hard-coded secret and a high inline-assembly finding.

**Status.** DONE

---

## Unit 5 — Scan targets and sessions

**Goal.** Promote a scan from one in-memory text buffer to a session over a set
of named targets, with a stable target identity and reproducibility record.

**Status.** TODO

---

## Unit 6 — Machine-readable report formats

**Goal.** A deterministic, versioned serialization of a scan report (per finding
and in aggregate) that another vessel or agent can consume without reparsing
prose.

**Status.** TODO

---

## Unit 7 — Code analyzers

**Goal.** Code-specific analyzers beyond literal patterns: dangerous calls,
unbounded operations, missing checks, and unsafe-language constructs.

**Status.** TODO

---

## Unit 8 — Permission and capability analyzers

**Goal.** Scan declared permissions, sigils, capabilities and grants for
excessive, missing or ambiguous authority, using the same finding model.

**Status.** TODO

---

## Unit 9 — Configuration and dependency analyzers

**Goal.** Scan configuration documents and dependency manifests for insecure
defaults, known-vulnerable versions and provenance problems.

**Status.** TODO

---

## Unit 10 — Runtime and live-surface scanning

**Goal.** Scan running system state (vessels, open surfaces, loaded artifacts)
through the semantic control plane rather than static files.

**Status.** TODO

---

## Unit 11 — Operating-system integration

**Goal.** Run Archon as a supervised vessel and expose scans as discoverable
rites through the shared semantic control plane, so human and machine clients
receive the same model and audit.

**Status.** BLOCKED — [ADR 0092](../docs/adr/0092-run-nil-on-anomalous-with-a-native-interpreter.md)
selects the direction: run NIL on Anomalous by natively compiling the NIL
interpreter (the C++ `ny::VM`) to the Anomalous ABI, and interpret editions.
Implementation is gated on qualifying that bounded native build and its service/
authority boundary; per-program AOT native compilation is deferred with a note
in that record. The concrete gap is recorded in
`notebook/Development/2026-10-15 Archon on Anomalous Build Path Note.md`: the
kernel has no NIL interpreter, `nycompiler` has no Anomalous output mode, and
the `--native` ELF it emits is neither the ELF adapter's sectionless deployment
subset nor complete enough for Archon's intrinsics (file I/O is unsupported
today).

---

## Open questions

- Is *Archon* a canonical term for `docs/nomenclature.md`? That introduces a
  platform-level abstraction, so it needs an ADR before code depends on it.
- Where do rules come from: compiled into the tool, loaded from a data file, or
  both? A rule is currently a record literal.
- What is the conformance oracle for scan correctness, and which existing
  analyzer (if any) does Archon replace or complement?
- What authority does a scanner need to inspect targets it does not own, and how
  is that authority attenuated and audited?
- How are findings disclosed to agents without leaking the secrets they report?
- Literal rules over-report: a bare `becomes` or `password` matches comments and
  identifiers. What precision bar must a rule meet before it is enabled by
  default, and how is it measured against a fixture corpus?
- How does a Matrix/NIL edition reach the kernel at all: an interpreter vessel, a
  native build in the ELF adapter's deployment subset, or a bounded NIL loader?
  See the build-path note; each is an ADR-sized decision.
