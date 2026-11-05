# Archon

Archon is the Anomalous scanning and security-assessment subsystem: the
one-stop tool for scanning code, configuration, permissions, dependencies,
runtime state, and other surfaces for security-relevant issues. It is written
in **Matrix**, the second surface language over the shared NIL runtime, and it
targets the project-wide runtime at `runtime/`.

Archon is deliberately broader than a signature virus scanner. Its semantic
model treats a **finding** as a first-class, machine-readable object — severity,
confidence, category, exact location, provenance rule, evidence and guidance —
so the same report can drive a human interface, an agent query, or, later, an
operating-system policy decision. A finding is not a log line: it is data a
machine can reason about.

Archon does not ship yet. This directory is its scaffold and living build
record. Matrix pages use the `.mat` shorthand (`.matrix` is also accepted). For
now Archon is a standalone host-side tool; operating-system integration comes
only after the model, the analyzers, and an accepted decision record exist.

## Runtime compatibility

The commands below retain a historical SDK contract. See
[optional runtime requirements](../docs/optional-runtime.md) before running them.
Compatibility with the current public NIL framework is not qualified. The
language sources are preserved for research and porting; no SDK is bundled.

## Build and run

Archon compiles against Anomalous's shared runtime at `runtime/`. Refresh it
from the repository root if it is missing:

```sh
NY_REPO_DIR=/path/to/compatible-sdk-source ./scripts/refresh-ny-runtime.sh
```

Then, from this directory:

```sh
make smoke          # compile and run the hello page, asserting its output
make model-smoke    # exercise the finding semantic model
make engine-smoke   # scan a sample target with sample rules and assert findings
make cli-smoke      # scan testdata/insecure.mat with the real CLI and assert the report
make check          # all of the above
make hello          # compile and run the hello page
make scanner-cli SCAN_ARGS=path/to/file.mat   # run the CLI on your own target
```

Overriding `SCAN_ARGS` runs the standalone CLI:

```sh
make scanner-cli SCAN_ARGS="src/archon/cli/scanner-cli.mat"
# or directly, targeting one or more paths:
runtime/nyinterpreter archon/build/scanner-cli.nil testdata/insecure.mat
```

Each argument is a target. The kind comes from the extension (`.mat`/`.matrix`
is Matrix; anything else is generic code), and the rule catalog adds the
matching rule set. Multiple paths are scanned in order. The CLI exits non-zero
when it is given no path or cannot read one.

Override the runtime location with `NY_RUNTIME_DIR=/path/to/runtime`.

## Layout

- `src/`        Matrix books (pages) that make up Archon.
- `testdata/`   fixtures the smoke tests scan, with intentionally insecure code.
- `Makefile`    builds and runs the pages against `runtime/`.
- `roadmap.md`  the unit-by-unit plan this scaffold follows.
- `matrix-issues.md`  Matrix-language friction found while building Archon.

Books are grouped by concern:

- `archon.model.*`   the semantic model: severities, confidence, categories,
  locations, findings, rules, targets and reports.
- `archon.engine.*`  the scanning engine: text patterns, rule application and
  the scanner that turns a target and a rule set into findings.
- `archon.rules.*`   rule sets and the catalog that selects them by target kind.
- `archon.cli.*`     the standalone command-line scanner.

A book lives at a path mirroring its dotted name under `src/`
(`book archon.model.severity` is `src/archon/model/severity.mat`). Only the
entry page is named on the compiler command line; everything it `read`s is
resolved through `-I src`.

## Status

Units 1–4 (semantic model, first rule engine, Matrix rule set and the
standalone CLI). See [roadmap.md](roadmap.md).
