# 2026-10-15 Archon Scanner Scaffold Log

This entry records the scaffold for the new Archon project: the finding model,
rule engine, and first text scan, all written in Matrix against the shared
project-wide Ny runtime.

## Why

Archon is intended to be the Anomalous one-stop scanning and security-assessment
subsystem. Rather than just a signature-based scanner, it treats a finding as a
first-class, machine-readable object so that humans and agents can reason about
the same result. The user requested it be written in Matrix, live under its own
`archon/` folder, and follow the same setup as Oculus against the shared runtime
SDK. This entry captures the initial standalone scaffold and the first end-to-end
scan slice.

## What changed

- Added `archon/` with:
  - `README.md` — purpose, build/run, layout, status.
  - `Makefile` — builds pages through `runtime/`, defines `hello`, `smoke`,
    `model-smoke`, `engine-smoke`, `check`, `clean`; `smoke` asserts the hello
    banner.
  - `roadmap.md` — units 1–10 with tasks, evidence and open questions. Unit 1–3
    land with this commit; Unit 10 (OS integration) is BLOCKED pending an ADR
    for the new platform-level abstraction.
  - `matrix-issues.md` — three Matrix-language friction points observed during
    development, including the `ny.lang.text::split_lines` → `vec<text>` type
    mismatch with Matrix `array of text`.
  - `src/hello.mat` — smallest runnable page (book `archon.hello`).
  - `src/archon/model/` — semantic model books:
    `severity.mat` (choice + rank + name), `confidence.mat`, `category.mat`,
    `location.mat`, `finding.mat`, `rule.mat`, `target.mat`, `report.mat`, and
    `smoke.mat` exercising the model.
  - `src/archon/engine/` — rule engine books: `patterns.mat` (split-lines
    implemented in Matrix using tier-one helpers; `line-contains`), `rules.mat`
    (rule-matches, finding-from-rule), `scanner.mat` (scan, summarize), and
    `smoke.mat` performing an end-to-end text scan with three rules across three
    categories.

- Wired into the repository:
  - Added `archon-smoke` and `archon-check` targets to the root `Makefile`.
  - Added `/archon/build/` to `.gitignore`.
  - Updated root `README.md` to list `archon` in the sub-projects table and add
    a short description in the Matrix section.

- Tested manually via the new targets:
  - `make -C archon smoke` — asserts the hello banner.
  - `make -C archon model-smoke` — compiles the model, runs it and asserts
    severity/category/location outputs.
  - `make -C archon engine-smoke` — runs an end-to-end scan with three pattern
    rules across categories (Code, Security, Permission) and asserts the machine-
    readable report line: `target=engine/sample.c findings=3 highest=critical
    rules=3`.
  - `make -C archon check` — runs all three.

## Model and engine

- The model defines `severity` (Info/Low/Medium/High/Critical), `confidence`
  (Speculative/Low/Moderate/Likely/Certain), `category` (Code/Security/Permission/
  Configuration/Dependency/Runtime/Data), `location` (target/path/line/column/
  excerpt with path:line:column summary), `finding` (first-class record with
  level/certainty/domain/place/guidance), `knowledge-rule`, `scan-target`, and
  `scan-report` (machine-readable summary). This vocabulary is broad enough to
  represent issues on any surface, as the user intended.
- The engine performs a simple literal substring scan line-by-line, producing
  findings that inherit the rule's severity/confidence/category and including
  the matched line as excerpt. A machine-readable summary is exposed as
  `report-summary`. The sample scan matches three findings: a hard-coded secret
  (Security, Critical), an unbounded copy (Code, High), and a world-writable
  permission (Permission, High).

## Language notes

- `ny.lang.text::split_lines` returns `ny::lang::collections::vec<text>`, which
  Matrix cannot consume as `array of text` (no `.len` member and type mismatch).
  The workaround is `archon.engine.patterns::split-lines` implemented in Matrix
  using `ny.lang.text::char_at`, `substring` and `index_of_from`.
- Value-position `consider` requires a final unguarded wildcard arm even when all
  choice variants are covered. The model adds `otherwise` clauses as defaults.

## Runtime context

Archon compiles and runs against the shared project-wide Ny runtime at
`runtime/` (the same Matrix-capable development SDK established for Oculus).
The hello and smoke targets confirm the build/run contract works.

## Limits

- This is a standalone, host-side scaffold. Operating-system integration is
  explicitly out of scope for this increment and is marked BLOCKED pending an
  accepted decision record (Archon introduces a new platform-level abstraction).
- The initial analyzer is a literal pattern matcher; deeper analyzers (structure,
  dataflow, capability analysis) are scheduled for later units.
- The "any kind of issue anywhere" promise is modeled semantically, but live
  runtime/system scanning remains TODO (Unit 9).
- The generated reports are human- and agent-readable but not yet serialized into
  a versioned wire format (Unit 5).

## Evidence

- `make -C archon check` succeeds: smoke (hello), model-smoke, engine-smoke all
  pass, including explicit grep assertions for the machine-readable report line.
- Book graphs compile with dependency resolution through `-I nystd -I src`, with
  only the entry page on the command line.

## Next steps

1. Determine whether "Archon" should be added as a canonical term in
   `docs/nomenclature.md` and initiate an ADR for the scanner subsystem before
   OS integration (Unit 10 gate).
2. Implement Unit 4 (scan sessions/targets beyond a single in-memory text) and
   Unit 5 (deterministic serialized report format) to make Archon agent-friendly.
3. Add the first non-literal analyzers (Unit 6: code) with tests.
4. Extend the model to support findings from non-textual targets without losing
   line/column semantics.
