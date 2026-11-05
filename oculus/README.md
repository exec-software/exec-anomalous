# Oculus

Oculus is the Anomalous desktop system: the session shell, window manager,
compositor, workspace model, input routing, and appearance layer. It is written
in **Matrix**, the second surface language over the shared NIL runtime, and it
is intended to replace the current bounded Rust desktop/compositor reference.

Oculus does not ship yet. This directory is its scaffold and living build
record. Matrix pages use the `.mat` shorthand (`.matrix` is also accepted).

## Runtime compatibility

The commands below retain a historical SDK contract. See
[optional runtime requirements](../docs/optional-runtime.md) before running them.
Compatibility with the current public NIL framework is not qualified. The
language sources are preserved for research and porting; no SDK is bundled.

## Build and run

Oculus compiles against Anomalous's shared runtime at `runtime/`. Refresh it
from the repository root if it is missing:

```sh
NY_REPO_DIR=/path/to/compatible-sdk-source ./scripts/refresh-ny-runtime.sh
```

Then, from this directory:

```sh
make smoke      # compile and run the hello page, asserting its output
make hello      # compile and run the hello page
```

Override the runtime location with `NY_RUNTIME_DIR=/path/to/runtime`.

## Layout

- `src/`   Matrix books (pages) that make up Oculus.
- `Makefile` builds and runs the pages against `runtime/`.
- `roadmap.md` the unit-by-unit plan this scaffold follows.

Smoke and conformance checks are currently driven from the `Makefile`; a
dedicated `tests/` directory arrives with the first conformance unit.

## Status

Unit 1 (hello and project skeleton). See [roadmap.md](roadmap.md).
