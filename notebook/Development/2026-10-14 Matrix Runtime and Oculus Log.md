# 2026-10-14 Matrix Runtime and Oculus Log

This entry records the change from a per-component, symlinked Ny SDK to a single
project-wide Ny runtime that carries the Matrix front end, and the first Oculus
scaffold built on it.

## Why

Matrix is a second surface language over the NIL runtime, implemented as a front
end inside `nycompiler` and selected by the `.matrix`/`.mat` extension. The
project's existing SDK generation (`456a9a11…`, activated as the `eidolon/sdk`
symlink) predates that work and rejects `.matrix`/`.mat` input with parse
errors. Oculus — the future desktop system — is to be written in Matrix, so the
first step was to give Anomalous one Matrix-capable runtime that every
sub-project can consume.

## What changed

- Added `scripts/refresh-ny-runtime.sh`. It builds and validates a complete Ny
  SDK from the sibling Ny checkout and copies the resolved generation into a
  real `runtime/` directory at the repository root. Private, generated state
  lives under `.runtime/`:
  - `.runtime/sdk` is the builder's atomically swapped activation symlink;
  - `.runtime/store/versions/<manifest-digest>/` holds immutable generations.
  The Ny CMake build is reused incrementally from the Ny checkout's `build/`.
- Retired the project-local path: deleted `eidolon/tools/refresh_ny_sdk.sh` and
  the untracked `eidolon/sdk` symlink, and repointed `eidolon/` (Makefile,
  test resolvers, README) at `../runtime/`.
- Repointed the opt-in Ny object proof (`tools/xtask/src/ny_object_artifact.rs`)
  at `runtime/` and re-pinned the generation and the three consumed tool
  digests.
- Updated `.gitignore` (ignore `/runtime/`, `/.runtime/`, `/oculus/build/`, and
  drop the old `eidolon/.sdk/` and `eidolon/sdk` entries), the root `README.md`,
  ADR 0016's project-wide SDK contract, and ADR 0049's pin.
- Added the Oculus scaffold: `oculus/README.md`, `oculus/Makefile`,
  `oculus/roadmap.md`, and `oculus/src/hello.mat`, plus a root
  `make oculus-smoke` target.

## Runtime identity

- Ny source revision: `2a45b0bb3891db6e32db5bc9d8786223c95c9687`
- Worktree: **dirty** (in-progress Matrix front end); the manifest records
  `worktree_dirty: true` and `qualification.mode: development`.
- Manifest digest / generation:
  `c541621251d8502d7d7c547d24f10322bc894e5943dc21a1abaaed59c71fd385`
- `nycompiler` SHA-256: `1550ac380dbb50a4c7b36e9f69ca9f63bf71cb888bbe7b7da59b2d18174c3f3f`
- `nyassembler` SHA-256: `3be1dfc1d6929fb4924fc86b31062c91908c4533f743be5d0c253291baa3378a`
- `nyinterpreter` SHA-256: `031d1ceb15098959cd1c8b1ec499f0413454f200751e08b871901e2e7140dc84`

This is explicitly a development runtime, not a language release declaration.
Every consumed artifact is still pinned by digest in `manifest.json`.

## Validation

- The Ny checkout's own suite passed 380/380 before packaging.
- `make -C eidolon check-ny` passes 26/26 against `runtime/`. This required
  relaxing `tests/test_ny_native_cli.py`, which previously required
  `worktree_dirty: false`; it now requires a valid pinned revision and a boolean
  dirty marker, and prints an explicit development-SDK notice. All digest pins
  are unchanged in strictness.
- `oculus/src/hello.mat` compiles and runs:
  - `runtime/nycompiler --no-bundle -I runtime/nystd --output hello.nil src/hello.mat`
  - `runtime/nyinterpreter hello.nil` prints
    `Oculus: hello from the shared Anomalous runtime`.
- `make oculus-smoke` passes.
- `cargo xtask ny-object-inspect` and `cargo xtask ny-object-image` reproduce
  the pinned scalar object/link/inspection profile and proof image.
- `cargo xtask ny-object-smoke` passed the TCG Intel-baseline boot at 96 MiB and
  320 MiB (free frames 24,149 -> 81,493, both scenarios, failure containment,
  and cleanup accepted). This target uses QEMU's default TCG acceleration; the
  actual-host KVM run and the wider regression matrix were not repeated here.

The remaining, expensive re-qualification step for ADR 0049 is the actual-host
KVM run and the full regression matrix against the new generation. Its result is
recorded in the roadmap and ADR 0049.

## Limits

- The runtime is a development SDK. A clean, tested, release-qualified runtime
  is still required before any release or compatibility claim.
- Matrix is a young front end; the runtime tracks uncommitted Ny work, so its
  exact digest is not reproducible from the committed tree alone.
- Oculus exists only as a scaffold; no desktop behaviour has moved to Matrix.
