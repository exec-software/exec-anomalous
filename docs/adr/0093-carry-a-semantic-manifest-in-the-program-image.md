# ADR 0093: Carry a semantic manifest in the program image

- Status: Accepted
- Date: 2026-10-15
- Scope: What an Anomalous program image states about itself before it runs,
  and how the loader uses that statement to bound the program's authority.

## Context

Anomalous already carries a semantic model, and it is good. `anomalous-abi`
defines an invocation envelope with real meaning: `discover`, `inspect`,
`preview`, `apply`, `cancel`, `compensate`, with `rite_rights` and
semantic-surface snapshots. A client — person or machine — can ask what an
operation means before asking for it.

All of that lives at the **invocation** boundary. The **image** boundary
carries none of it:

- `native_application_artifact` fixes geometry (`image_base = 0x400000`,
  8 pages RX, 2 pages RW) and nothing else.
- `anomalous-elf-adapter` admits a sectionless ELF64 subset and states plainly
  that it conveys "no implied authority".
- The `ANRT` startup descriptor is 128 bytes: a role, a scenario, a 32 KiB
  arena, two handles, and a document of at most 31 printable ASCII bytes.

So an agent handed an Anomalous executable today can learn nothing about what
it does, or what authority it will reach for, without running it. The project's
stated reason for having its own ABI rather than adopting one is that an
Anomalous program should be legible to a machine client at a level other
systems do not offer. That property is not in the artifact yet.

### Why the interpreter forces the question now

ADR 0092 selects the NIL interpreter as the execution route for NIL/Matrix
programs. That decision makes the gap urgent rather than theoretical, because
**an interpreter is a generic image**. The same bytes run every program. Nothing
that can be learned by inspecting the interpreter says anything about the
program it is about to run.

Admitting the interpreter under today's model therefore means admitting a
universal execution engine whose image states nothing about any workload it will
ever carry. Measured against the route Anomalous already has — ADR 0049, where
every import is resolved by inspecting the artifact — that is a regression in
what the system knows before it runs something.

The resolution is structural: the thing that varies per program is the NIL
edition, so the edition is what must carry the statement, and the interpreter's
granted authority must be a **ceiling** rather than a claim.

### What a NIL edition already contains

Two facts were verified against the current compiler on this box.

**Authored intent survives compilation.** A Matrix `context "..."` annotation
reaches the artifact as `@ai_context(...)`. The language already has the
feature; it simply stops at the image boundary.

**The authority surface is derivable, not merely declarable.** A NIL edition
names every intrinsic its linked code can reach, as `INTEROP_CALL intrinsic
<name>`. A page that reads a file yields `file`, `path`, `console`, `convert`,
`array` — 277,615 bytes of artifact. A page that adds two numbers and touches
nothing yields **no intrinsics at all, in 846 bytes**.

That difference is the point. The surface is computed from the artifact rather
than asserted by its author, so it cannot be understated by a program that would
rather not say. It over-approximates — reading a book admits that book's whole
intrinsic surface whether or not every entry is called — and over-approximation
is the safe direction for a bound: it can only ask for more than is needed.

## Decision

Define a **semantic manifest** as a required section of an Anomalous program
image, and make the loader enforce it.

**1. The manifest states, at minimum:**

- the **authority surface**: the set of service families the image may reach,
  derived from the artifact rather than declared by its author;
- the **intent**: authored, human- and machine-readable text describing what the
  program is for, carried from `@ai_context`;
- the **identity** of the artifact the surface was derived from.

**2. Authority is derived, and the derivation is reproducible.** A build
produces the manifest by inspecting the artifact. Anyone holding the artifact
can recompute the manifest and compare. A manifest that disagrees with its
artifact is not a warning; the image is refused.

**3. The loader intersects, and the interpreter is a ceiling.** The interpreter
is admitted once, with the maximum set of services it may ever grant. Each
edition carries its own manifest. What runs is the **intersection**, and an
edition asking for anything outside the interpreter's ceiling is refused before
execution rather than failing at the call.

**4. Over-approximation is accepted, under-approximation is not.** A manifest
may name more authority than a run uses. It may never omit authority the image
can reach.

**5. This record does not set the size limits.** The bounded geometry plainly
has to grow for an interpreter — a 469 KiB edition against a 64 KiB image limit
and a 32 KiB arena — but those are numbers, and they belong with the
interpreter's build profile. This record is about what an image *says*.

## Consequences

- A machine client can read an image's purpose and reachable authority without
  running it, which is the property the project claims and does not yet have.
- Anomalous gets an artifact format that is its own rather than inherited: the
  current image contract is Rust's freestanding conventions plus fixed
  addresses, and a semantic manifest is not expressible in those terms.
- Every image gains a build step and a refusal mode. A build that cannot derive
  a manifest cannot ship an image.
- The interpreter becomes admissible on the same terms as everything else:
  bounded by what it may grant, not trusted because it is ours.
- `@ai_context` becomes load-bearing, which it is not today. Matrix drops a
  `context` annotation that does not precede a declaration, silently. That
  becomes a defect to fix rather than a quirk.
- Derivation over-approximates, so a program that reads one book for one
  function admits that book's whole intrinsic surface. Narrowing it is a
  reachability analysis, and a later refinement; the bound is sound meanwhile.
- The manifest is a stable artifact format, so changing it later needs its own
  record. ADR 0000 requires this one for the same reason.

## Alternatives

- **Author-declared authority.** A program states what it needs and the build
  trusts it. Rejected: it is a promise rather than a property, and ADR 0049
  already set the precedent that the artifact is what gets inspected.
- **Enforce only at the call boundary.** The invocation ABI already refuses
  unauthorized operations, so an image could say nothing and be stopped in the
  act. Rejected: it gives a machine client nothing to reason about beforehand,
  and "find out by running it" is precisely what an agent cannot afford.
- **Put the manifest only in the interpreter.** Simplest, and wrong for the
  reason this record exists: the interpreter is generic, so its manifest cannot
  describe the workload.
- **Defer until after the interpreter runs.** Tempting, and it inverts the
  dependency: admitting a universal execution engine without a per-program
  statement is the decision that would be hard to walk back, because everything
  built on it would assume the weaker contract.
- **Adopt an existing format's metadata** (ELF notes, a signed manifest
  standard). Not rejected, and deliberately not chosen here: the encoding is a
  later question, and settling it now would mix an encoding choice into a
  contract decision.

## Follow-ups

- Choose the manifest's encoding and placement, and whether it is signed.
- Specify the service families the surface is expressed in, and their mapping
  from NIL intrinsic names.
- Fix Matrix dropping a `context` annotation that precedes a non-declaration.
- Size the interpreter's image and arena, with its build profile (ADR 0092).
- Decide whether reachability narrowing is worth its complexity, once there is
  a real corpus of editions to measure.

Written for the runtime side of the NIL-on-Anomalous work described in
`notebook/Development/2026-10-15 NIL Runtime into Anomalous Handoff.md`, whose
open questions this record answers in part. ADR 0092 selects the execution
route; this one states what the thing being executed must say about itself.
