# Contributing

Anomalous is in its research bring-up phase. Small, evidence-backed changes are
more valuable than broad implementations whose invariants are unclear.

## Before changing architecture

Open an issue or discussion before work that changes a privilege boundary,
kernel responsibility, wire format, boot contract, package model, compatibility
promise, or platform target. Record an accepted direction as an ADR in
[docs/adr](docs/adr).

An architectural proposal should state:

- the problem and non-goals;
- security and failure boundaries;
- invariants the implementation must preserve;
- human and machine interaction effects;
- observability and recovery behavior;
- alternatives considered; and
- the evidence that will validate the decision.

## Development loop

~~~sh
make check
make smoke
make fault-smoke
make circle-smoke
make memory-smoke
make interrupt-smoke
make shell-smoke
make normal-exclusion
~~~

Boot changes should also be exercised with make virtualbox and include the
relevant COM1 event transcript in the pull request.

Keep pull requests reviewable. Tests belong at component boundaries, and
hardware-facing changes require a VM integration check whenever possible.

Use [notebook](notebook) for dated observations, experiment evidence, research
questions, and handoffs. Working notes may remain uncertain when clearly
labeled; normative architecture and settled platform decisions belong under
`docs/` and the ADR process.

## Public source and attribution

Execute Software and community contributors are recognized in
[CONTRIBUTORS.md](CONTRIBUTORS.md). You may add your preferred name to the
community section when submitting a contribution.

Keep workstation paths, private service names, credentials, and unredacted
runtime captures outside the repository. Scan the tracked/non-ignored tree,
staged changes, and reachable Git history before publication:

```sh
python3 tools/audit_secrets.py --scanner /path/to/gitleaks \
    --report-dir /path/to/private/new-audit-directory
```

The helper requires Gitleaks 8.30.1 and writes redacted reports outside the
checkout. Reports are private review material; do not attach them to public
issues. A clean scan supplements manual review and does not prove an absence
of secrets, intellectual-property issues, or vulnerabilities.

## Style

- Project-defined identifiers use snake_case, including types, variants,
  constants, functions, modules, fields, and assembly labels.
- Rust and C-family sources use K&R braces and four spaces, never hard tabs.
- External identifiers keep the spelling required by their ABI or dependency.
- Run rustfmt; do not hand-format around it.
- Public interfaces explain units, ownership, lifetime, error behavior, and
  compatibility expectations.
- Occult terms come from [docs/nomenclature.md](docs/nomenclature.md). Do not
  invent a synonym when a canonical term exists.

## Unsafe Rust and assembly

Safe Rust is preferred, but safety is an engineering property rather than a
language claim. Every unsafe block and assembly routine must document:

- why safe Rust is insufficient;
- memory, register, alignment, and privilege assumptions;
- inputs, outputs, clobbers, and ownership transfer;
- the invariant that makes the operation safe; and
- how the path is tested or inspected.

Keep unsafe boundaries narrow. Never parse rich, attacker-controlled formats in
the kernel when a constrained vessel can do it.

## Human and machine parity

A feature is incomplete if an important effect exists only in the GUI or only
in an agent API. New system operations should plan for one typed rite with
consistent discovery, sigil checks, preview, audit, cancellation, and result
semantics across clients.

AI-generated or AI-assisted code has exactly the same authorship, licensing,
review, test, and safety requirements as any other contribution.

## Commit hygiene

Write imperative commit subjects and explain why a change exists. Avoid mixing
mechanical cleanup with behavior. Generated build artifacts under build/ and
target/ are never committed.

By contributing, you agree that your contribution is licensed under the MIT
license in this repository.
