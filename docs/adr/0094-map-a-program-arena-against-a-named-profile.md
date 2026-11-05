# ADR 0094: Map a program arena against a named profile

- Status: Accepted 2026-10-15 for mapping and launching a program-bearing
  task. Reviewed and revised the same day; qualified by host fixtures and two
  guest boots. Accepted for the launch contract only: no edition is executed
  and none is mapped.
- Date: 2026-10-15
- Builds on: ADRs 0049, 0090, 0092 and 0093
- Scope: How the kernel's owned-mapping core decides what arena a task may be
  given, and how a program-bearing task gets one larger than a native
  application's.

## Context

ADR 0092 selects a native interpreter as the execution route for NIL and
Matrix programs. ADR 0093 requires the thing being executed to state its
authority before it runs, and `program_descriptor` now carries that statement
across the launch boundary in an `ANPG` wire form. The remaining gap is
physical: no kernel path can map the task that would read one.

The owned-mapping core admits one arena geometry, and it admits it by literal:

```rust
// owned_execution.rs
if task_count != 2
    || arena_bases.iter().any(|base| !matches!(*base, 0x804000 | 0x814000))
{ return Err("owned_arena_profile"); }
```

`prepare_native_arenas` therefore admits exactly two tasks at exactly two
addresses, `prepare_profile` admits `code_pages` of 1 or 8, the arena's length
is the native application's fixed `arena_byte_count` of 32 KiB rather than a
parameter, and the entire `arena_bases` argument exists only under
`#[cfg(feature = "native_runtime_test")]`.

What `ANPG` does and does not establish is worth stating plainly, because
the two are easy to conflate. It carries an edition reference and a
`granted_services` ceiling across the launch boundary. It does **not** derive
or enforce ADR 0093's semantic manifest: nothing yet recomputes an authority
surface from an artifact and refuses an image that disagrees with it. This
record is about making such a task mappable, which is upstream of that work
and does not substitute for it.

A program-bearing launch does not fit that, and not by accident.
`minimum_arena_bytes` is 64 KiB because an interpreter sizes its own heap,
where a native application's needs are known at build time and fixed. The
arena is a **bounded range** in `program_descriptor` for the same reason. So
the descriptor cannot be satisfied by the only geometry the kernel can map.

Retrofitting a scenario into `native_runtime_profile` was tried and does not
work: that profile grants 32 KiB per task, with `arena_frames` of 66 and
evidence pinned to `data_frames` 48, `leases` 12 and `zeroed_bytes` 262144.
Widening it cascades through all of those, and it contradicts the direction
already recorded for the interpreter's launch -- a new profile rather than a
widened bounded one.

## Decision

Make the arena geometry a **named profile** that the mapping core validates,
rather than a pair of addresses it recognises.

**1. Geometry becomes an explicit, validated parameter.** A profile names the
arena's base and length, the code page count, and the task count. The core
admits a launch by checking it against a profile rather than by matching
literals.

**2. A profile is checked before it maps anything.** Page-aligned, non-empty,
within the user address limit, and disjoint from the image, data, descriptor
and stack regions it will sit beside. That check runs *before* the task's own
extents are considered, so an incoherent profile is reported as the profile's
fault rather than as the task's -- the discipline ADR 0093's deployment
profile already established for the ELF image boundary.

**3. Existing geometry is one named profile, its values stay fixed constants,
and it is held by independent golden tests.** The current two bases, 32 KiB
arena and eight code pages become the native application's profile, and
`0x804000`/`0x814000` remain fixed constants inside it rather than becoming
ordinary validated values.

The control is **not** a comparison between the existing entry point and that
profile. Review caught this: if the wrapper and the expectation both resolve
to the same profile, the test proves delegation and nothing else, and the two
can drift together while it still passes. The relevant precedent --
`parse == parse_for(anel_v1)` at the ELF boundary -- is insufficient here for
that reason.

So the profile is pinned by golden values restated independently of it:
admitted and rejected bases, the 32 KiB arena length, eight RX pages, four
mappings and five reservations, page permissions, and exact resource counts.
Every previously accepted combination is preserved, not one representative
example -- a single passing case would not notice a profile that narrowed.

**4. Validation covers the whole geometry, not just the arena.** Guard and
null reservations, checked arithmetic and page rounding, RW/NX against the
image's RX and the W^X property, and the data and table frame budgets
including supervisor residency. A geometry that satisfies the arena bound and
violates any of these is refused.

**5. The profile kind is selected by the controller, never by the launch.**
The kind is chosen before preparation, retained with the prepared owner, and
**revalidated in the copy, cancel and cleanup paths** rather than trusted once.
This is the security property the record exists to protect: an `ANPG`
descriptor sits in the task's own writable memory, so a program that rewrites
its descriptor must not be able to select a profile or widen an authority.
Launch data describes a launch; it never chooses the terms of one. ADR 0049's
precedent holds -- what the kernel admits is what the kernel inspected.

**6. A program launch is a distinct profile, not a widened one.** It carries a
single task, a 256-byte `ANPG` launch region, and an arena bounded by
`minimum_arena_bytes` and `maximum_arena_bytes`. It is gated by its own
feature, so a kernel image built without it cannot map a program task at all.

A single-task launch also reaches `physical_access::registration_slot`, which
today permits one task only under `owned_workspace_test`. That path takes an
explicit program-feature gate and its own regression coverage rather than
inheriting another profile's allowance.

**7. This record does not set the interpreter's arena size.** Consistent with
ADR 0093, the shape is decided here and the number comes when there is an
edition to measure. The profile bounds it; the build chooses it.

## Consequences

- A program-bearing task becomes mappable, which is what ADR 0092's execution
  route and ADR 0093's manifest both require before either can be exercised.
- The owned-mapping core gains a parameter and a validation step. It is shared,
  security-critical code reached by every profile family, so the equivalence
  test in decision 3 is the control that matters: no existing launch may be
  admitted by different bounds than before.
- A new feature means a new kernel image configuration and its own smoke, and
  one more build the qualification set has to cover.
- An arena base stops being self-evidently safe by being one of two constants
  and becomes safe by being checked. That is a better property, but it moves
  the guarantee from inspection to validation, so the validation is the thing
  to review.
- Profile validation is pure and runs **before allocation**, inside the
  synchronous preparation callback that ADR 0091 requires to return before
  guest entry. So it composes with the phase separation rather than competing
  with it: no destructor cleanup, no borrowed-allocator escape and no guest
  dispatch happens inside that callback.
- An incoherent geometry therefore maps nothing -- it is refused before
  anything is allocated. This is **not** a claim that a launch can never be
  partially admitted, which an earlier draft of this record wrongly implied.
  Allocation and population failures occur after validation has passed, and
  those deliberately retain partial leases so the failure is an explicit,
  retryable cancellation rather than an implicit unwind. Refusing a profile and
  failing to acquire memory are different outcomes and keep different
  guarantees; only the first is all-or-nothing.

## Qualification

Measured, not derived. The figures this record reasoned to in draft were
afterwards produced by the running kernel and agree with the derivation.

Host fixtures, `cargo test -p kernel --lib`, 618 tests:

- A prepared program owner reports `(0x804000, 65536)`. The same owner under
  the native application's profile reports 32 KiB, which is the defect this
  record exists to remove. Negative-tested by asserting 32 KiB.
- Nine resident tables, bracketed rather than read from a private field: nine
  admits, eight returns `table_budget_exhausted`, and the user plan's own
  table count asserts at eight beside it. Negative-tested by asserting that
  eight admits. `resident_plan` adds exactly one to the user plan's count, and
  the borrowed physical alias is not an owned table -- which is what that
  `+ 1` is not.
- Eight plan tables at both admitted bases and under both profiles, with zero
  headroom, so a geometry crossing one more 2 MiB boundary is refused rather
  than quietly mapped.
- The native refusals hold against the program geometry: a non-writable arena,
  an executable one and an unadmitted base are each refused before any access,
  with allocator accounting and the memory counters unchanged.

Guest boots, `cargo xtask smoke-native-program`, at 96 MiB and 320 MiB, both
admitted bases in each:

```
data_frames 25 | table_frames 9 | leases 6
arena_bytes 65536 | zeroed_bytes 131072 | calls 2 | exits 1
grant_restated true | editions_executed 0
```

`data_frames` is 1 code + 2 data + 16 arena + 2 stack, plus one four-page
transition stack. `calls` is exactly two -- startup and exit -- because the
gate admits nothing else yet; a third call would mean the task reached for
something. The six granted words are read back through the kernel's own alias
and compared against what the kernel wrote, never reported by the task.

What this qualification does **not** establish, stated in the emitted evidence
rather than left to a reader: `edition_mapped` and `console_transport` are
both false and both required to be present. No interpreter exists, no edition
bytes are mapped, and a scenario that grants `console_write` mints a handle
with no transport behind it, so every gate operation except startup and exit
is refused.

## Alternatives

- **Lower `minimum_arena_bytes` to 32 KiB** so a program launch fits the
  existing geometry. Rejected: it weakens a published bound to suit the test
  harness rather than the workload, and ADR 0092's interpreter will exceed
  32 KiB regardless, so it buys an immediate demonstration and defers the same
  problem in a worse position.
- **Widen the native application's arena for every task.** Rejected: it
  charges every bounded application for the interpreter's needs, and the fixed
  32 KiB is a property worth keeping for a task whose requirements are known
  at build time.
- **Add a scenario to the existing runtime profile.** Rejected on measurement:
  the cascade through frame counts and pinned evidence above, and a recorded
  decision against it.
- **Prove the launch contract host-side only.** Not rejected as a step, and
  cheap, but it leaves unproven the one thing that is actually in doubt --
  whether the kernel can map and launch such a task.
- **Let the descriptor's arena range drive the mapping directly**, with no
  profile. Rejected: the descriptor is launch data written into a task's
  writable memory, and ADR 0049's precedent is that what the kernel admits is
  what the kernel inspected. A profile is the kernel's statement; a descriptor
  is the launch's.

## Follow-ups

- Done: the program vessel, its kernel profile (`native_program_profile.rs`,
  attached by one `cfg_attr` path arm, no new delegation points), its xtask
  artifact and its smoke oracle.
- `console_transport` over an application channel, once a granted console
  handle exists on the far side of a real launch.
- The interpreter's arena and image sizes, with its build profile (ADR 0092).
- ADR 0093's manifest derivation and loader enforcement, which this record
  makes reachable and does not provide.
- The authenticated source and suspended-caller APIs (`read_loader_source`,
  `validate_suspended_caller`, `runtime_mapping`) and the Linux nine-region
  profile stay unchanged here; a later mapping-candidate slice sits beside
  them.

Written for the NIL-on-Anomalous runtime work. ADR 0092 selects the execution
route, ADR 0093 states what the executed thing must say about itself, and this
one makes the task that would run it mappable.
