# ADR 0008: Isolate foreign-platform compatibility

- Status: Proposed
- Date: 2026-09-08

## Context

Running Linux, Windows, and macOS applications is central to the long-term goal,
but each platform has different technical, security, and licensing constraints.
Folding all foreign behavior into the kernel would undermine modularity.

## Decision

Develop each compatibility veil independently and publish its tier: source/SDK,
binary ABI or syscall personality, API translation, isolated virtualization, or
remote execution. Veils live in user space or VMs and integrate through narrow
semantic bridges. Linux is the first practical experiment; Windows and macOS
have separate gates, with explicit legal review for macOS.

## Consequences

Compatibility will be incremental and may never be uniform across platforms.
Applications retain visible provenance, deviations, authority, and isolation
status instead of creating a misleading universal-compatibility claim.
