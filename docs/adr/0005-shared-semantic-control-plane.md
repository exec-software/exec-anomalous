# ADR 0005: Use one typed semantic control plane

- Status: Proposed
- Date: 2026-09-08

## Context

Pixel automation and ad hoc privileged agent tools cannot provide a durable,
auditable operating model across local and remote devices.

## Decision

Expose versioned objects and rites. An invocation carries principal, trace,
idempotency, requested behavior, arguments, and required sigils. Mutating work
supports discovery, preview, policy evaluation, an effect-bound approval digest,
progress, cancellation where meaningful, typed results, and compensation where
possible.

The GUI, command environment, remote device surface, and exec-hag are clients of
this protocol. Transport can vary; authorization and audit meaning cannot.

## Consequences

Schema evolution and partial failure become core design problems. In exchange,
people and machines can reason about the same operations without screen scraping
or a privileged automation backdoor.
