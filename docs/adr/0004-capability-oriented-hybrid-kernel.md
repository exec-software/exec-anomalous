# ADR 0004: Start with a capability-oriented hybrid kernel

- Status: Proposed
- Date: 2026-09-08

## Context

The system needs strong isolation and replaceable, debuggable components without
assuming that every microkernel boundary will meet future latency needs.

## Decision

Keep memory protection, scheduling, interrupts, time, sigil primitives, bounded
IPC, and essential failure telemetry in the kernel. Place drivers, filesystems,
networking, package management, policy, UI, observability, and compatibility in
supervised user-space vessels. Move a mechanism only when measured latency or a
privilege invariant requires it.

## Consequences

Services can be restarted and inspected independently, but IPC, supervision,
resource accounting, and driver isolation become foundational work. The
boundary remains evidence-driven while its security invariants stay explicit.
