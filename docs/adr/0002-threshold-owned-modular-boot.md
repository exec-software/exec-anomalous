# ADR 0002: Own a modular boot path with Threshold

- Status: Accepted
- Date: 2026-09-08

## Context

Anomalous is intended to explore execution and hardware relationships that
cannot yet be predicted. A loader designed primarily to boot existing operating
systems would make a foreign contract the first permanent boundary in the
system.

## Decision

Anomalous owns its boot loader, named Threshold, and does not use GRUB. Firmware
frontends and architecture transitions are narrow modules that converge on a
versioned `boot_omen` contract. The initial path uses x86-64 BIOS in VirtualBox
and QEMU because it offers a small inspectable starting point; UEFI and other
architectures are future frontends, not separate kernel contracts.

Threshold remains a loader and recovery boundary, not a second kernel. Storage,
verification, platform discovery, display, diagnostics, and transition modules
must publish their assumptions and fail explicitly when unavailable.

## Consequences

The project owns more low-level code and security responsibility. In return,
boot metadata, recovery, measurement, AI-era hardware needs, and future formats
can evolve without inheriting an unrelated loader architecture.
