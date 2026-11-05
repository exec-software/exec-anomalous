# 2026-10-15 Archon CLI and Matrix Rules Log

This entry records the second Archon increment: a real standalone command-line
scanner, the first language-targeted rule sets (generic plus Matrix-specific),
and the catalog that selects them. It continues
`2026-10-15 Archon Scanner Scaffold Log.md`.

## Why

The scaffold proved the finding model and a first text scan but could only scan
an in-memory sample. To be useful — and to have something to exercise on the
kernel in a VM later — Archon needed to run as a command-line tool, read real
files, and carry the Matrix-specific rules the user asked for first (Matrix is
the target language for the new OS). This increment delivers that without yet
crossing into privileged or OS integration.

## What changed

- Added rule books under `src/archon/rules/`:
  - `generic.mat` (`archon.rules.generic`) — language-agnostic patterns:
    unbounded copy (`strcpy(`), unbounded read (`gets(`), hard-coded secret
    (`password`), hard-coded key (`api_key`), weak hash (`md5(`), command
    execution (`system(`), world-writable path (`chmod 777`).
  - `matrix.mat` (`archon.rules.matrix`) — Matrix-specific constructs grounded
    in the Matrix guide: inline assembly (`assembly for`), runtime intrinsics
    (`note intrinsic`), manual memory (`allocate `), unbounded loop
    (`repeat forever`), raw text block (`begin text`), broad import
    (`read everything from`), agent self-description (`context `), unchecked
    cast (`becomes`).
  - `catalog.mat` (`archon.rules.catalog`) — `rules-for kind` and
    `rule-count-for kind`: generic rules always, Matrix rules when the kind is
    `matrix`. One place decides which rules apply.
- Replaced the placeholder CLI with a real one in
  `src/archon/cli/scanner-cli.mat` (`archon.cli.scanner`):
  - Reads its targets from `argv` (`ny.lang.system::arg_count`/`arg`).
  - Reads each file with `ny.lang.io::read_text`.
  - Infers the kind from the extension (`.mat`/`.matrix` → `matrix`, else
    `code`).
  - Scans, prints one `finding-summary` line per finding and one
    `report-summary` line per target.
  - Prints usage and exits 2 with no paths; exits 1 when a path cannot be read.
- Added `testdata/insecure.mat`, an intentionally insecure Matrix fixture with a
  known finding set.
- Makefile: `scanner-cli` (run with `SCAN_ARGS=...`) and an asserting
  `cli-smoke`; `cli-smoke` is part of `check`.
- Docs: roadmap gains a new Unit 4 and the later units shift to 5–11;
  `archon/README.md` and the root `README.md` describe the CLI; this log.

## Evidence

`make -C archon check` runs all four smokes. `cli-smoke` scans
`testdata/insecure.mat` and asserts a critical hard-coded secret, a high
inline-assembly finding, and the report line:

```
target=testdata/insecure.mat findings=6 highest=critical rules=15
```

At the command line the tool behaves like a program: no arguments prints usage
and exits 2, an unreadable path exits 1, and multiple paths scan in order.

## Language notes

- **A book's declared name must match its file path.** The first attempt put
  `book archon.rules.matrix` in `matrix-codestyle.mat`; imports of it failed and
  the compiler reported `Undefined identifier` at each *call site* rather than
  "module not found". Renaming the file to `matrix.mat` fixed it. Recorded in
  `matrix-issues.md`.
- `value` is a reserved word, so it cannot be a binding or parameter name; the
  `int_to_string_i64` parameter is `n`, and loop/index bindings use other names.
  Recorded in `matrix-issues.md`.
- `ny.lang.io::read_text` returns a Ny `Result<text, text>`, and matching it from
  Matrix works as `when Result.Ok holding source` / `when Result.Error holding
  message` once `ny.lang.core` is imported. `try_arg` is matched as
  `Option.Some`/`Option.None`. This boundary is genuinely usable.
- The interpreter passes only user arguments — `argv` excludes the program name,
  the same as `arg_count`/`arg` imply — and `main`'s returned number becomes the
  process exit code. Verified by running the compiled page directly.

## Limits

- Still a literal pattern matcher: a bare `becomes` or `password` will match
  comments and identifiers, so precision is low and the noisy rules carry low
  confidence. Reducing false positives is future work (data-flow and structure
  analyzers).
- File discovery is not recursive; the CLI scans exactly the paths it is given.
- The report is human-readable text plus the stable summary line, not yet a
  versioned wire format (now Unit 6).
- No OS integration (now Unit 11); still gated on an accepted decision record.

## Next steps

1. Decide the "Archon" nomenclature and ADR question before any OS integration.
2. Unit 5: scan sessions over many named targets with a reproducibility record.
3. Unit 6: deterministic, versioned serialization of findings and reports.
4. Start replacing low-precision literal rules with structure/data-flow
   analyzers (Unit 7), beginning with the Matrix constructs already named here.
