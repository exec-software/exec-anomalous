# ADR 0009: Separate AI semantics from hardware mechanisms

- Status: Proposed
- Date: 2026-09-08

## Context

Anomalous aims to make exec-hag a native interface for AI work while allowing
workloads to use accelerators efficiently. Binding model or agent concepts
directly into the kernel would freeze a rapidly changing field and enlarge the
most privileged attack surface.

## Decision

Keep exec-hag and model semantics above the kernel ABI. Expose small mechanisms
for memory placement and pinning, bounded zero-copy sharing, asynchronous work
queues, scheduling and accounting, interruption, and IOMMU-confined DMA.
Accelerator familiars translate these mechanisms for devices. Reliquary records
models, kernels, provenance, requested resources, and governed context; sigils
authorize each boundary crossing.

## Consequences

Hardware efficiency must be measured across several layers, and accelerator
support requires strong isolation and accounting. The kernel stays evolvable
while “bare metal” performance remains compatible with policy, attribution,
debuggability, and recovery.
