# 2026-10-15 NIL Runtime into Anomalous — Runtime Owner Reply

Answers to the six open questions in
`2026-10-15 NIL Runtime into Anomalous Handoff.md`. The handoff is accurate and
its five ABI corrections all hold; this adds the decisions and one correction
of emphasis.

## The emphasis I would change

The handoff treats the ABI gaps as sizing: the image is too small, the arena is
too small, the startup contract is the wrong shape. That is all true and it is
the mechanical half.

The half that is not sizing is that **the image says nothing about itself**. The
semantic model lives entirely at the invocation boundary — `invocation_kind`,
`rite_rights`, semantic-surface snapshots. The image boundary carries fixed
geometry, a role, a scenario, two handles and 31 bytes of document, and the ELF
adapter says in its own words that it conveys "no implied authority". An agent
handed an Anomalous binary learns nothing from it without running it.

The interpreter is what makes that urgent. **An interpreter is a generic image**:
the same bytes run every program, so nothing learned by inspecting it describes
the workload. Admitting it under today's contract means admitting a universal
execution engine that states nothing about anything it will carry — weaker than
ADR 0049, where every import is resolved by inspecting the artifact.

ADR 0093 records the decision. Two facts behind it, verified against the current
compiler:

- A Matrix `context "..."` annotation already reaches the artifact as
  `@ai_context(...)`. The chain exists and stops at the image.
- A NIL edition names every intrinsic its linked code can reach. A page that
  reads a file yields `file`, `path`, `console`, `convert`, `array` in 277,615
  bytes; a page that adds two numbers yields **no intrinsics at all, in 846
  bytes**. So the authority surface is *derived from the artifact*, not declared
  by its author, and cannot be understated by a program that would rather not
  say. It over-approximates, which is the safe direction for a bound.

## The six questions

**1. Bigger native task profile, or a new profile?** A new one. The bounded
32 KiB task's whole value is that it is provable; widening it to fit an
interpreter destroys the thing it exists for. Add a second profile and leave the
first untouched, the way a derived ABI is added beside a qualified one rather
than replacing it.

**2. Embed the edition, or read it through a file service?** File service.
Embedding makes every program a fresh image, which removes the interpreter
route's one advantage over ADR 0049, and `scanner-cli.nil` at 469 KiB does not
fit the geometry regardless.

**3. Extend `ANRT`, or a new descriptor?** New descriptor. `ANRT` is 128 bytes
with tight invariants and a version already in the field; an interpreter needs
an edition reference, an argv equivalent and an exit status. Extending it
changes every existing validator for the benefit of one new consumer.

**4. Which services first, and multiplexed?** Console write, then bounded file
read, then time — exactly the order that walks `hello.nil` to `scanner-cli.nil`.
Separate handles first: the two-handle shape generalises to N, while a
multiplexed channel needs a framing protocol that is new surface to qualify
before anything it carries can be.

**5. Rust `no_std` reimplementation, or freestanding C++?** Rust, and the
toolchain is not the reason. `vm.cpp` is 6,330 lines using `<filesystem>`,
`<fstream>`, `<iostream>`, `<thread>` and `<locale>`, with 207 `throw` sites.
Option B is not blocked on a freestanding C++ path so much as on the VM's
composition — it would be a rewrite either way, so it should be a rewrite in the
language the kernel is already proved in.

The divergence risk is the real objection and it has a concrete answer: the
acceptance gate is the existing corpus. `matrix.execution` is 32 fixtures with
exact expected output, three of which compare emitted NIL schemas against
hand-written Ny twins. A second VM ships when it passes that corpus unmodified.
That is a gate rather than an aspiration, and it has caught several real defects
in the C++ VM this week.

**6. Must an edition be verified before it runs, and who holds that authority?**
Yes, and ADR 0093 is where it goes. The loader intersects the edition's manifest
with the interpreter's ceiling and refuses outside it, before execution rather
than at the call.

## Order of work

1. ADR 0093 accepted or amended. It is the contract everything else assumes.
2. Interpreter build profile: image and arena sizes, the new startup descriptor,
   console write. Enough to run `hello.nil`.
3. Bounded file read, then time. Enough to run `scanner-cli.nil`.
4. Manifest derivation in the build, and loader enforcement.

Steps 2 and 3 are the handoff's corrections 1–5 and are mechanical once the
shape in step 1 is agreed. Keep producing editions meanwhile — `hello.nil` is
the first target and it already exists.
