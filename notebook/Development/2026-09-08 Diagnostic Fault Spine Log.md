---
title: "Diagnostic Fault Spine Log — 2026-09-08"
date: 2026-09-08
tags: [development, kernel, exceptions, gdt, tss, idt, qemu, virtualbox]
status: complete
domain: foundations
---

# Diagnostic Fault Spine Log — 2026-09-08

## Intent

Make CPU faults attributable before attempting memory ownership or a ring-3
circle. A direct privilege transition without a diagnostic path would turn
ordinary descriptor and mapping defects into silent resets.

## Implemented

- Replaced Threshold's inherited descriptor state after Rust entry with a
  kernel-owned GDT containing 64-bit code, data, and TSS descriptors.
- Added fixed 16 KiB stacks for future ring transitions, double-fault IST 1,
  and NMI IST 2.
- Installed gates for all 32 architectural exception vectors.
- Added assembly entries that normalize processor error codes, preserve general
  registers, clear the direction flag, and establish the Rust calling
  convention.
- Added a fatal Rust dispatcher with static exception names, origin, register
  frame attribution, CR2 capture, and decoded page-fault cause bits.
- Added a nested-fault stop path. Fault recovery is deliberately absent.
- Added a test-only exact-match invalid-opcode injection and the bounded
  `make fault-smoke` QEMU harness.
- Added the fault smoke test to repository guidance and GitHub Actions.

## Evidence

| Check | Observed result | Artifact or command |
| --- | --- | --- |
| Host checks | 3 ABI, 15 kernel, and 3 xtask tests passed; all Clippy targets passed with warnings denied | `make check` |
| Threshold boot regression | Kernel installed the fault path and still emitted `boot.ready` | `make smoke` |
| Deliberate fault | Vector 6 arrived from the exact armed `ud2` address with CS `0x0008`; the handler emitted `kernel.fault_test.passed` | `make fault-smoke` |
| Shell regression | Injected commands completed and the final prompt remained live with command-text redaction intact | `make shell-smoke` |
| VirtualBox normal image | Kernel emitted `kernel.fault_path.ready`, entered the bootstrap shell, and remained live | `build/virtualbox.log` |

The implementation checkpoint is commit `03b852a`. Generated logs and images
remain ignored local artifacts.

## Boundaries

**Observed:** kernel-owned descriptors, TSS loading, 32 IDT gates, exact
invalid-opcode attribution, and normal boot/shell regression.

**Not claimed:** a circle, vessel, recoverable exception, safe external
interrupt handling, safe NMI delivery, stack-overflow containment, address
privacy, or a production crash pipeline. NMI remains masked until its dedicated
entry has deterministic injection and nested-fault evidence.

## Next proof

Define the first circle boundary in an ADR, then build a fixed test-only ring-3
probe with supervisor-only kernel mappings, user RX code, user RW+NX data/stack,
guard pages, and a deliberate forbidden kernel read. The gate is an attributed
user-origin page fault followed by a clean supervised outcome; without a timer,
it will not prove denial-of-service containment.
