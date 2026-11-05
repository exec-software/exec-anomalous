# Repository guidance for coding agents

Anomalous is pre-alpha operating-system research for people and machines. Read
`VISION.md`, `docs/architecture`, accepted records in `docs/adr`, and
`notebook/Overview/Current State.md` before an architectural change.

## Non-negotiable project decisions

- Threshold is the Anomalous-owned modular boot loader. Do not introduce GRUB.
- Ny is the official language direction under ADR 0045. Existing verified Rust
  remains authoritative until qualified Ny replacements exist; ADR 0016's
  adoption gates and a separate privileged-migration ADR still apply. Assembly
  and unsafe Rust stay narrow and document invariants.
- Project-defined code identifiers use `snake_case`, including types, variants,
  constants, labels, and test names.
- Rust and C-family code use K&R brace placement and four-space indentation.
- Occult terms use exactly the meanings in `docs/nomenclature.md`.
- Human and machine clients use the same semantic control and policy model.
- Neumorphic visual work must also meet accessibility and nonvisual-state needs.
- Broad compatibility, security, and performance claims require measured proof.
- Keep working observations, experiments, and open questions in `notebook/`;
  keep normative architecture and accepted decisions in `docs/`.

## Version control

Commit and push small, tested checkpoints regularly. Do not accumulate multiple
completed development slices in an uncommitted working tree. Preserve unrelated
work and keep each checkpoint reviewable.

## Validation

Run all commands before handing off a code change:

```sh
make check
make smoke
make fault-smoke
make circle-smoke
make control-smoke
make rite-smoke
make scheduler-smoke
make supervision-smoke
make shell-vessel-smoke
make memory-smoke
make interrupt-smoke
make input-wait-smoke
make shell-smoke
make semantic-surface-transport-smoke
make semantic-presentation-smoke
make normal-exclusion
```

Boot changes should additionally be exercised in VirtualBox when the environment
supports it. Never commit generated files from `build/` or `target/`.

`cargo build --workspace` does not build the freestanding vessels and its
failure is not a defect to fix. It builds them for the host, where the linker
is `cc` and a bare `--build-id=none` is rejected because `cc` forwards linker
arguments only through `-Wl,`. A vessel's real target is `x86_64-unknown-none`,
where `rust-lld` is invoked directly and takes the bare spelling, so the build
script is correct as written. Adding the `-Wl,` prefix to satisfy the workspace
build breaks the vessel instead, which has already happened once. Build a
vessel through its `cargo xtask` target, or with
`cargo build -p <crate> --target x86_64-unknown-none`.

`kernel` sets `[lib] test = false`, so neither `cargo check --tests` nor
`cargo check --all-targets` builds its lib test target: a green run of either
has compiled none of the `#[cfg(test)]` modules, and all of them live in the
lib. Only naming the target forces it, with `cargo test -p kernel --lib`
(`--no-run` to stop at the compile). This has twice made new host fixtures
look verified when they had never been compiled, once hiding a four-argument
call passed three arguments. A check is not coverage on this crate.

New privilege boundaries, platform abstractions, stable formats, or reversals
of accepted direction require an ADR. Make failures structured, attributable,
and recoverable; do not hide important effects in either a GUI-only or an
agent-only path.
