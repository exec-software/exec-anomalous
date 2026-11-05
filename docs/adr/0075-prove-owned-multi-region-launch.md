# ADR 0075: Prove owned multi-region launch

- Status: Proposed; bounded native mechanism qualified
- Date: 2026-10-07
- Related: ADR 0041, ADR 0042, ADR 0043, ADR 0046, ADR 0047, ADR 0073, ADR 0074

## Context

The selected Linux Antimatter path needs native memory and launch mechanisms
that can represent ordinary Linux executable layouts. ADR 0074 inspects actual
`true`, interpreter, libc and Electron files in userspace-compatible portable
code. Current launch profiles admit only three or four fixed regions, one
initialized executable interval and user trees that already contain the low
supervisor ancestry. Main plus interpreter already require eight LOAD regions
before their stack. None of those restrictions is an ELF parsing concern.

## Proposed decision

Add the default-off `owned_multi_region_test` native mechanism proof. Preserve
the default eight-region planning capacity and existing fixed constructors.
The opt-in build supports sixteen planning slots and sixteen exact executable
intervals; the trusted launch profile in this increment selects exactly nine
regions, at most 128 user pages, 32 combined table pages and sixteen immutable
source spans. These operation budgets do not impose path/name limits or define
a stable artifact, launch wire format or application ABI.

The native input describes page-aligned regions and permissions, reservations,
immutable byte-copy spans, initialized executable intervals, initial RIP and
an explicit guarded stack region/RSP. Source spans must be nonempty, checked
against overflow, wholly contained in one admitted region and mutually
nonoverlapping. Every declared executable interval must be initialized by a
complete source span and wholly contained in a user RX/nonwritable region.
Stack admission retains the existing guard, headroom and alignment rules. The
physical proof accepts immutable linked rodata as source authority; a future
userspace loader requires a separately qualified copied/owned source boundary.

`task_bounds` and `user_entry_plan` retain the exact initialized interval set.
Entry must belong to it. Gate, timer, selected-frame and saved-context return
checks use the same set, rejecting inter-region holes and executable-page
padding. Existing single-interval getters describe the interval containing
entry, never an enclosing address hull. This does not prevent user code from
branching inside an executable mapped page: hardware permissions are page-sized.
Kernel return admission and hardware execution permissions are distinct checks.

An explicit `resident_plan::new_with_ancestry` combines user requirements with
missing low supervisor PDPT, PD and PT branches. Existing user table indexes
remain unchanged. Synthesized branches are supervisor-only; shared ancestors
retain user access only where user descendants need it. The old constructor
continues to require its prior low branches. Every table entry, including absent
gaps, has an exact expected target/permission value. After activation, accessed
bits are permitted on present entries and dirty only on writable leaves.

The retained launch owner stores the explicit stack index and admitted mapping
count. Before allocation it revalidates the admission against its exact mapping,
requires at least two initialized RX regions, and selects the nine-mapping
circle policy explicitly. Legacy circle tables retain limits three/four. All
regions are populated and read back completely, including source prefixes,
unaligned initialization, zero holes, BSS and page slack. Ownership precedes
allocation/population; failures retain leases and accurate attachment progress.
Nothing is published as a task until explicit commit.

Activation and teardown retain the existing BSP-only, IF-clear/NMI-masked
ownership contract. The runtime restores bootstrap CR3/RSP0 before abandoning
the private transition stack, withdraws scheduler contexts, unregisters roots,
revalidates mapping contents, retires tasks/circles, clears/releases all owned
storage, and finally removes the physical alias. A failed clear retains the
unreleased ownership for retry. No ELF headers, interpreter paths, auxv policy,
Linux calls, relocation rules or library lookup enters the kernel.

Preparation constructs the retained runtime directly. Unpublished rollback and
live dispatch use separate non-inlined lifetimes, preserving the existing 64 KiB
bootstrap stack and 16 KiB preparation-headroom requirement. Admission identities
remain absent until real tasks are committed, before activation or publication.

After slow unpublished initialization, a proof-local one-shot interrupt operation
rearms the existing PIT mode/divisor immediately before first entry. It requires
initialized interrupts, IF clear and the existing timer-only PIC masks. It does
not change the timebase, coarse tick, divisor, masks or normal EOI path; an
already latched PIC edge may still deliver normally. This prevents accumulated
preparation-time PIT reinjection from consuming every test quantum before either
user loop runs. Repeated or invalid calls perform no port writes. The operation
is not available to admitted tasks or a general timer-control ABI.

## Physical proof

Two native GPR-only payload pairs use the nine-region geometry observed for
`true` and its loader: main at bias `0x40000000`, interpreter-shaped regions at
`0x60000000`, and a 64 KiB stack at `0x7fff00000000`. Each task owns 88 user
pages, ten table pages and four private transition-stack pages. The two tasks
therefore retain 204 frames in 22 leases and eighteen circle mapping children.
These payloads are mechanism fixtures; the proof does not run Linux binaries.

Each task enters one RX region, calls the other with two live return addresses,
passes a checked gate there, then jumps back and passes a second checked gate.
Source prefixes of 55 and 32 bytes remain outside the admitted initialized
executable intervals. A separate twenty-byte unaligned RW source and all other
bytes undergo full initialization/readback. Private canaries and a noninitial
RSP survive 64 timer switches. One task then faults with an attributed UD2 and
its peer continues for sixteen timer callbacks before exit. Eight forbidden
return addresses are checked against actual admission; this is not a claim of
hardware page-fault testing at those addresses.

Before entering either task, two complete unpublished preparations inject a
failure at each transition-stack clearing boundary. Each retains ownership and
then cleans up on retry. Host tests separately exercise malformed intervals and
sources, exact budgets, wrong mapping/admission bindings, absent supervisor
ancestry, entry corruption and allocation/population/cleanup failure boundaries.

## Qualification boundary

Require strict ordered serial evidence with exact fields, physical QEMU proof
at 96/320 MiB under TCG and KVM where available, a VirtualBox observation where
supported, all sixteen repository checks and affected native/ELF regressions.
Builds and VMs remain serialized under the memory guard. Verify default image
exclusion and compare saved baseline bytes. The dated notebook records exact
source, artifact, memory and failure evidence. The bounded qualification is
recorded in the [owned multi-region launch log](../../notebook/Development/2026-10-07%20Owned%20Multi-region%20Launch%20Log.md).

This increment supplies a native prerequisite for unchanged Linux loading. It
does not provide a general loader endpoint, TLS, extended CPU state, Linux
filesystem/mapping services, dynamic relocation or Electron execution. The
next application acceptance remains unchanged `true` with its loader/libc,
followed by the services required by the real Antimatter application.
