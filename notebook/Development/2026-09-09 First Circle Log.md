---
title: "First Circle Log — 2026-09-09"
date: 2026-09-09
tags: [development, kernel, circle, ring3, paging, isolation, qemu, virtualbox]
status: verified
domain: foundations
---

# First Circle Log — 2026-09-09

## Intent

Test the smallest useful x86-64 protection-domain boundary before introducing
allocators, tasks, scheduling, IPC, production system calls, or vessel policy.
Ring-3 entry alone is insufficient: this experiment must prove selected page
permissions, exact denial attribution, destructive teardown, and clean
replacement.

## Decided scope

**Accepted:** [ADR 0011](../../docs/adr/0011-prove-the-first-circle-with-a-fixed-x86-64-prototype.md)
defines a removable `circle_test` feature with fixed addresses and reports. It
uses a private page-table root whose kernel hierarchy remains supervisor-only,
plus one high lower-canonical user window containing RX code, RW+NX scratch and
stack pages, and absent guards. NX and supervisor write protection must be
enabled and read back before ring-3 entry.

The experiment has three generations:

1. observe clean scratch, report ring-3 entry, then attempt to read a present
   supervisor-only kernel canary and accept only page-fault error `0x5` at the
   exact labelled instruction;
2. rebuild from verified-zero storage, report entry, then jump to the present
   NX stack and accept only page-fault error `0x15` at that exact target; and
3. rebuild from verified-zero storage again and make one terminal ring-3 report.

Every accepted fault must also match its lifecycle phase, CR2, user code and
stack selectors, stack bounds, and generation scratch marker. A feature-only
DPL-3 interrupt gate accepts fixed numeric reports and no pointer, string,
buffer, operation, or authority.

## Teardown invariant

After each denied or completed generation, the implementation must return to a
saved kernel stack, restore the original CR3, clear the private paging pages and
code, data, stack, and privilege-stack storage through volatile writes, verify
every byte as zero, and only then construct the next generation. The denied
user frame is discarded; execution after either forbidden operation is a test
failure.

## Recorded result

**Observed:** ADR 0011 was added at checkpoint `603381e`. The implementation,
strict QEMU harness, normal-image exclusion, and initial notebook scaffolding
were pushed at checkpoint `1d11e3b`.

Both QEMU and VirtualBox completed the full lifecycle. QEMU saved page-fault
RFLAGS as `0x2`; VirtualBox correctly added the architectural resume flag and
saved `0x10002`. The predicate permits only that single difference. Both
emulators produced error `0x5` at the exact copied supervisor-read instruction
and canary address, error `0x15` at the exact NX stack target, three verified
teardowns, and the terminal pass event.

## Required evidence

| Check | Required observation | Status |
| --- | --- | --- |
| Host checks | Descriptor and gate privilege bits, paging permissions and guards, one-field fault-predicate mutations, report ordering, and volatile clear behavior | Passed: 22 circle-feature kernel tests and 10 xtask tests |
| Circle smoke | One strictly ordered trace for protection setup, three mappings and entries, two exact denials, three teardowns, terminal pass, and debug-exit status 33 | Passed in QEMU |
| Exclusion | Normal image contains no test gate, payload, report path, or terminal circle event | Passed before every circle smoke run |
| Regressions | `make check`, `make smoke`, `make fault-smoke`, and `make shell-smoke` remain green | Passed |
| VirtualBox | Feature image demonstrates the same bounded sequence on the local development VM | Passed; normal shell image restored and running |

Duplicate, reordered, missing, fatal, nested, after-denial, reset, signal,
timeout, and early-exit observations all fail the experiment.

## Boundaries

**Not claimed:** general circles or address-space ownership, processes, threads,
scheduling, preemption, syscalls, IPC, sigils, arbitrary loading, peer
isolation, supervision, restart policy, or a vessel. The fixed recovery
trampoline and numeric gate are diagnostic machinery, not production ABI.

The proof also does not establish global W^X, speculative-side-channel
resistance, kernel page-table isolation, SMEP, SMAP, DMA confinement,
multiprocessor teardown, or denial-of-service containment. With no timer, a
looping user payload still cannot be preempted.

## Next foundation

The experiment is complete and ADR 0011 is accepted. The next architectural
slice should define general physical-frame and virtual-address-space ownership,
including boot-memory normalization, reserved extents, allocation invariants,
mapping ownership, and deterministic reclamation tests. It must not promote the
fixed gate, addresses, static pages, or recovery trampoline into platform ABI.
