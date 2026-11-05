# ADR 0076: Prove owned loader sources

- Status: Accepted for the bounded native source proof
- Date: 2026-10-07
- Related: ADR 0019, ADR 0046, ADR 0048, ADR 0073, ADR 0074, ADR 0075

## Context

ADR 0074 describes Linux image and startup bytes without execution authority.
ADR 0075 qualifies native multi-region memory and entry ownership, but its
physical preparation accepts only immutable linked kernel rodata. A userspace
loader needs to supply bytes without leaving a consumer dependent on its live
address space, buffers or privileged alias.

Keep ELF parsing, path lookup, relocation and Linux policy outside the kernel.
Copying source bytes is a native ownership mechanism. Possession of source bytes
must not grant an address space, destination permissions or executable entry.

## Decision

Add a default-off `owned_source_test` proof over the existing multi-region
runtime. A move-only `owned_source` retains one allocator-owned contiguous
extent, an exact uploader task incarnation, a trusted nonzero grant, logical
byte length, append cursor and lifecycle state. The embedding assigns fresh
grants; a copied number or task identity alone grants no access.

Source byte/page budgets are explicit operation inputs. The physical fixture
uses two 7048-byte snapshots with an 8192-byte/two-page budget each. These are
proof budgets, not global file, path or image limits. The inspected unchanged
`true` and interpreter need larger source budgets in a later integration.

Allocation clears and verifies complete pages, including slack, and retains
any acquired lease before a fallible bind. Import accepts only a nonempty
sequential chunk of at most 4096 bytes. Request metadata is checked before
reading uploader bytes. The embedding authenticates the exact current task,
attached scheduler context, retained root and privilege stack, circle profile,
page permissions and every touched translation under sole-BSP IF-clear/NMI-
masked exclusion. The nine-mapping policy is explicitly selected by the trusted
retained launch profile; observed mapping counts never select policy.

A separately acquired supervisor scratch copies one complete preflighted user
chunk. It is cleared and read back after every result and never becomes retained
source storage. Source writes are read back before advancing the cursor. A
backend failure can dirty an unaccepted suffix, but does not advance the cursor;
retry must replace and verify the same complete chunk. Privileged hardware
faults remain fatal rather than a recoverable foreign-pointer promise.

Seal requires every declared byte and revokes further append authority. A
sealed reader borrows the actual owner, preventing mutation/release during
population. Native source spans contain destination, source offset and length;
no uploader virtual address or privileged alias is retained. The plan validates
complete source bounds, whole-region destinations, nonoverlapping writes and
initialized executable interval coverage. Destination admission and commit
retain ADR 0075's independent permission, stack and ownership checks.

The coordinator retains sealed source leases outside the producer execution
owner. Producers and all their task/context/root/alias storage are retired
before fresh consumers are prepared. A new physical-access owner supplies a
scoped alias for each source read. Consumer population copies and reads back
all destination bytes, zero holes and slack. Failed reads/population leave the
consumer unpublished with its acquired ownership retained; source reads may
modify scratch on failure and callers discard it.

Cleanup withdraws source read/write authority before the first fallible clear.
A failed clear retains the unbound lease and release progress for retry. No
fallible automatic Drop releases storage. The source snapshot conveys bytes,
not authentication of an executable format, a signature or launch authority.

## Physical proof

Two producer tasks use the qualified nine-region native geometry. Each imports
1024 bytes from its main RX region, 1024 from its interpreter-shaped RX region,
and 5000 bytes from an unaligned RW buffer. Four successful imports and five
explicit denials exercise incomplete sealing, wrong grants, replayed offsets,
a copy straddling the end of a mapped region, and writes after sealing.
Host tests additionally reject stale task/circle generations, overflow, budget
violations, malformed spans, partial storage failures and failed cleanup.

After sealing, each producer overwrites and verifies its original data buffer.
A privileged sealed-source readback checks that the snapshot kept the original
pattern. The complete producer dispatch/fault/exit lifecycle then retires.
Fresh consumers receive code and data only from the sealed snapshots. They
check all 5000 pattern bytes in their new private mappings before entering the
existing sustained-dispatch proof. Both generations independently establish
64 preemptions, an attributed UD2, sixteen survivor callbacks and exact cleanup.

Each dispatch generation has its own one-shot rearm of the unchanged PIT mode
and divisor after slow preparation. The consumer rearm requires the first
operation to have been consumed. Producer retirement masks every external line
through an explicit one-shot quiesce transition without marking the initialized
backend failed. With IF still clear, a matching one-shot restore reopens only
IRQ0 before consumer preparation. Failed initialization remains terminal. No
timebase, timer divisor or user timer-control ABI is changed. Final source cleanup injects one failed clear per snapshot,
then retries and verifies complete resource restoration.

Producer and consumer preparation use separate non-inlined lifetimes. Bulk
source bytes live in accounted frames; copy scratch is supervisor BSS with an
exclusive acquisition guard. The existing bootstrap stack and preparation
headroom requirement remain unchanged. Emitted stack inspection is required
before physical runs because the preceding proof had a narrow margin.

## Qualification boundary

Require hostile host tests, freestanding lint and feature-precedence checks;
strict ordered serial evidence under QEMU TCG/KVM at 96/320 MiB and isolated
VirtualBox where supported; all sixteen repository-required checks and affected
multi-region/application/native-editor regressions. Serialize builds and VMs
under the memory guard, retain failed attempts and compare saved image baselines.
The dated notebook records final source/artifact hashes, emitted review, memory
observations and the exact qualification boundary.

This is an experimental native source/launch mechanism, not a stable upload
wire ABI or an ordinary Linux executable launch. Unchanged `true` still needs
integration of actual image/startup sources, task-owned TLS/CPU state, dynamic
mapping/protection and Linux filesystem/descriptor/startup services. The existing
Linux Antimatter/Electron application remains the product target.
