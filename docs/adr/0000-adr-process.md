# ADR 0000: Architecture decision records

- Status: Accepted
- Date: 2026-09-08

## Context

Anomalous expects long-lived interfaces across firmware, kernel, vessels,
devices, packages, compatibility systems, people, and AI clients. Decisions
need durable reasoning rather than oral history.

## Decision

Material architectural decisions are recorded in `docs/adr` with context,
decision, consequences, and alternatives. Status is one of Proposed, Accepted,
Superseded, or Rejected. Accepted records are immutable except for corrections;
a new ADR supersedes a changed decision and links both records.

An ADR is required for a new platform abstraction, privilege boundary, stable
wire or artifact format, compatibility promise, or reversal of a documented
principle.

## Consequences

Review has more up-front structure, while future contributors and machine
clients gain a searchable history of constraints and tradeoffs.
