# ADR 0006: Make artifacts and environments native system concepts

- Status: Proposed
- Date: 2026-09-08

## Context

AI-era software carries context, schemas, models, provenance, permissions, and
debug data that traditional executable and package boundaries treat as
afterthoughts. In-place package mutation is difficult to explain and recover.

## Decision

Define Reliquary as the versioned, signed native executable and package artifact
format. It holds execution records and payloads alongside governed context,
semantic schemas, requested sigils, resource intent, provenance, SBOMs, and
debug maps. Parsing rich sections occurs in constrained vessels.

Define Covenant as the package and environment transaction system. It resolves
a `covenant_manifest` into immutable content-addressed generations with complete
previews, explicit authority, atomic activation, and rollback. Lifecycle work
uses declared rites rather than ambient-root scripts.

## Consequences

Anomalous must build format tooling, migration rules, trust policy, garbage
collection, and import adapters. It gains one inspectable model for native code,
AI artifacts, system services, applications, and environments.
