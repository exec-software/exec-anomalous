# Ny vessel on the rite ABI

Working log for the slice that gets a Ny vessel booted by the kernel and
speaking the rite ABI, rather than a Ny object entered through an inspected
C-wrapper shape with trap stubs standing in for a runtime.

ADR 0049 qualified a bounded freestanding Ny object. Its costs were recorded
there and are worth restating, because every one of them was a property of
what the Ny toolchain emitted rather than of what the kernel required: a
narrow assembly shim implementing the *observed* ABI-version, context-begin,
context-end and failure hooks; fatal trap stubs for `strlen`, `calloc` and
`memcpy`, which the compiler imported unconditionally even for scalar source;
no way to disable the red zone; and an ordinary runtime that assumed hosted
allocation, OS services and TLS.

Those are now fixed on the NIL side, and the toolchain ships the runtime. This
log records what that makes possible here, and what it does not.

## What was blocking, measured rather than assumed

**There was no Ny runtime in this checkout at all.** `runtime/` did not exist,
so `ny-object-inspect`, `ny-object-image` and `ny-object-smoke` could not run
-- correctly, since ADR 0049 requires them to fail rather than skip when the
pinned generation is unavailable. Nothing Ny-related here was reproducible
locally.

**The refresher could not have produced one.** `scripts/refresh-ny-runtime.sh`
required an SDK containing `nycompiler`, `nyinterpreter`, `nyassembler`,
`nylinker`, `nylsp`, `nydebugger`, `nyschema`, `nyagent` and the directories
`nylib` and `nystd`. The NIL Framework SDK has named its tools `nil*` and its
directories `nillib`/`nilstd` since the rename, and now also ships `nilbuild`,
`aperture` and `niltest`. The default `NY_REPO_DIR` pointed at
`../exec-lang-ny`, which does not exist; the toolchain lives in
`../exec-nil-framework`. The script is updated to the current inventory.

**The SDK did not ship the one archive a vessel links.** A generation carried
`libnil.a` and `libnil.so` only; the freestanding runtime had no install rule,
so a vessel could not have linked it from a pin at all. Fixed upstream: a
generation now carries `nillib/native/libnilruntime_freestanding.a`, recorded
in the manifest by size and hash. The refresher checks for it at refresh time,
so a generation that lacks it is named as the problem instead of surfacing
later as a missing symbol during a vessel link.

**The archive was built for the wrong kind of target, twice over.** Both were
found by attempting a vessel link against the `.forbidden` guard in a linker
script, not by reading the source.

- It was position-independent, so the weak host failure hook was reached
  through a GOT. A `.got` is exactly what a fixed-address vessel profile
  rejects.
- It used SSE. The runtime has no floating point, but at `-O2` the compiler
  vectorized its zero and copy loops into `pxor`/`movups`/`movdqu`. ADR 0067
  keeps a GPR-only soft-float target, and the rite fixtures deny inherited
  x87/SIMD state by leaving CR0.TS set, so the first such instruction would
  fault. A consumer cannot opt out of what is already in the archive.

Both are fixed upstream (`-fno-pic`, `-mgeneral-regs-only`), each with a
tripwire in the NIL link contract so neither can return quietly.

## The channel, which turned out to be the interesting part

A Ny object publishes `__ny_entry` and nothing else. Its globals are not in
the symbol table -- `pub` does not change that -- so a host cannot reference
them, and `@interop` only accepts a closed allow-list of versioned libraries,
so Ny cannot declare or call a host symbol either. Measured, not inferred:

```
$ nm -g --defined-only vessel.o
0000000000000120 T __ny_entry
```

So the only things crossing the boundary are `__ny_entry`'s return value and
the hooks it calls. That rules out the obvious designs, and points at the one
that works: **the region is the channel.** The host installs memory with
`nyrt_heap_init`; Ny's string concatenation allocates from it; the host reads
the result back out of memory it owns. Ny never learns an address, and nothing
of this ABI enters Ny.

Ending the Ny context resets the allocation pointer and deliberately does not
scrub the region, which is what makes the read after `__ny_entry` returns
valid. That is now stated in the NIL contract rather than being a property a
consumer discovers.

## What exists here

`vessels/anomalous-ny-vessel/` -- `vessel.ny`, `entry.S`, `vessel.ld`.

The Ny half composes its message instead of naming a constant, because
composition is what needs the allocator, and returns 42. The Anomalous half is
the whole of the binding and is deliberately small: install the region, call
`__ny_entry`, take the message's length through `nyrt_str_length`, copy it into
the mailbox payload through `nyrt_mem_copy`, submit one `console_write`, then
`task_exit` carrying what Ny returned. It uses no libc and reaches memory only
through the runtime it links.

Rite bindings stay here, per ADR 0019 and as agreed with the NIL side: that
ABI is ours and is explicitly unstable, so it does not enter `nilstd`. What
NIL supplies is the capability -- a freestanding object, a runtime with no
hosted assumptions, and no imports we did not ask for.

The vessel's state lives in two words of its own `.bss`, because the gate
requires every GPR but R15 to be zero and nothing survives a submission in a
register.

## Measured

Built with the pinned generation (`nilcompiler --no-bundle --no-stdlib`, then
`nilassembler --unmanaged --freestanding --no-red-zone`) and linked against
`runtime/nillib/native/libnilruntime_freestanding.a`:

| Property | Value |
| --- | --- |
| Unresolved imports in the linked image | none |
| Entry address | `0x400000`, exactly |
| RX extent | 2,092 bytes text + 253 rodata, one page |
| Vessel state | 60 bytes at `0x800000` |
| Region | one page at `0x801000` |
| x87/MMX/SIMD instructions in the whole image | 0 |
| Relocations, dynamic section, GOT, PLT | none |
| `int $0xf3` sites | 2 |
| Forbidden sections | none; every linker assertion holds |

The sequence before the gate was then run on Linux against the same object and
the same pinned archive, which fixes the values a kernel harness should
assert:

```
payload_size   = 38
exit status    = 42
heap used      = 0
mailbox[40..]  = 'Anomalous vessel speaks through a rite'
```

`heap used = 0` with the bytes still readable is the context-end behaviour
described above, exercised rather than assumed.

## What this does not show

**No vessel has booted.** There is no kernel harness for this vessel yet, so
nothing here crosses `int $0xf3` under the kernel. What exists is a loadable
image that satisfies the profile a harness would impose, and the measured
values such a harness should check. The claim is "it links, and every artifact
property a vessel needs holds", not "it runs".

The rite path itself is independently known good: `make rite-smoke` passes on
this checkout, with the gate installed at vector 243, nine calls, a committed
16-byte `console_write`, and clean release accounting. That is the dispatcher
this vessel is written against, exercised by a hand-written vessel.

## The payload stage exists

`make ny-vessel-inspect` (`tools/xtask/src/ny_vessel_artifact.rs`) takes the
vessel from pinned source to a relocation-free carrier the kernel link can
consume. It is a separate module from `ny_object_artifact` on purpose: that one
is pinned to ADR 0049's generation and emission shape, and one pin cannot stand
for two contracts.

It refuses a generation whose manifest or whose `nilcompiler`, `nilassembler`,
`nilinterpreter` or freestanding archive hash has moved -- the archive
separately, so a generation that changed only that file is still rejected; an
object importing anything but the eight runtime-ABI names; a link that acquires
an undefined symbol or an unplaced section (`--no-undefined` with
`--orphan-handling=error`, so the linker script names everything it admits);
and an image that does not start at its page base, does not fit one page,
places its failure or return labels outside that page, moves its writable
layout, or lets the region reach the mailbox.

The interpreter runs the same source as an independent semantic oracle and its
exit status is required to be 42, so the native path is not the only thing
asserting what the program means. It passes with no output on either stream.

Payload: 2,349 bytes, carried by `x86_64_ny_vessel_payload_start/end`.

## The mapping budget, which shapes the harness

A circle admits `circle_mapping_capacity = 3` mappings, and
`rite_test` spends exactly that on code, mailbox and stack. A Ny vessel needs
its own writable data as well, which is a fourth thing, so the obvious layout
does not fit.

`map_owned_extent` is the way out: a multi-page extent is one mapping, which is
how `supervised_renderer_test` covers six pages with three. The vessel's
writable pages are contiguous and share permissions -- state at `0x800000`,
region at `0x801000`, mailbox at `0x802000`, all RW and NX -- so they map as a
single three-page extent:

| Mapping | Extent | Permissions |
| --- | --- | --- |
| 1 | `0x400000`, one page | user RX |
| 2 | `0x800000`, three pages: state, region, mailbox | user RW, NX |
| 3 | stack, kernel-chosen | user RW, NX |

The mailbox sharing an extent with vessel-writable memory is not a weakening:
the dispatcher already reads it through its own physical alias and copies
admitted bodies rather than trusting the vessel, and a vessel can write
anywhere in its own memory in any case. It is also the more honest statement of
what the pages are -- all of them are the vessel's.

Addresses stay below 4 GiB throughout because the object is not
position-independent; `rite_test`'s window at `0x6000_0000_0000` would overflow
the 32-bit relocations the small code model emits. The mailbox constant in
`entry.S` moved to `0x802000` for the same reason.

## It boots

`make ny-vessel-smoke`, TCG:

```
kernel.ny_vessel.ready             vector 243, grant 7, image_bytes 2557
kernel.ny_vessel.task.entered      mappings 3, user_rx + user_rw_nx, guards absent
kernel.ny_vessel.console.committed sequence 1, ok, 38 bytes, composed_by ny
kernel.ny_vessel.exited            sequence 2, ok, code 42, returned_from_ny
kernel.ny_vessel.released          65116/65116 frames, 0 owned, 0 active
kernel.ny_vessel.passed            calls 2, commits 2, terminal
```

A Ny object, entered through `__ny_entry`, linked against the shipped
freestanding runtime, booted by this kernel, composing its own message into the
region the kernel installed and sending it through the real rite gate. Recorded
as [ADR 0099](../../docs/adr/0099-boot-a-ny-vessel-on-the-rite-abi.md).

The kernel also reads the region back after the vessel exits and requires the
message to be there, NUL-terminated, as the vessel's first allocation -- so the
bytes the rite carried are the bytes Ny composed, not merely bytes of the right
length.

## Ny issues the rite itself

The first boot's binding ran before and after Ny's `main`, so the vessel could
not speak from inside its own control flow. That needed a capability Ny did not
have: a way to call a symbol the consumer defines. `@interop("host")` is now
that capability upstream -- the declared Ny function *is* an external symbol of
exactly that name, called with the ordinary C convention -- and the vessel uses
it:

```ny
@interop("host")
fn anomalous_rite_console_write(message: text) -> i64
end fn
```

`anomalous_rite_console_write` is ours, defined in `entry.S`. It takes a
pointer to NUL-terminated bytes -- which is what a Ny `text` is natively --
copies them into the mailbox payload through the runtime rather than libc,
submits one `console_write`, and returns the committed count. Nothing of the
gate, the mailbox or the vector is visible to Ny, which is the arrangement ADR
0019 asks for.

The vessel now writes twice, and the second write is the point:

```
kernel.ny_vessel.console.committed sequence 1, ok, 38 bytes, issued_by ny, from_region
kernel.ny_vessel.console.committed sequence 2, ok, 23 bytes, issued_by ny
kernel.ny_vessel.exited            sequence 3, ok, code 42
kernel.ny_vessel.passed            calls 3, commits 3, console_writes 2, issued_by ny
```

Ny sends the second line only because the first rite returned the count it had
passed. So the loop is closed in both directions: Ny → rite → kernel → result
→ Ny decides → rite again. That is a vessel acting on the ABI rather than
crossing it.

Two consequences worth recording.

**The stub must preserve what the gate destroys.** The gate requires every GPR
but R15 to be zero, and the stub is now a System V callee, so it saves and
restores `rbx`, `rbp` and `r12`-`r15` around the submission -- plus one extra
qword, because six saved registers leave the stack eight bytes short of the
16-byte boundary the runtime calls need. The committed length still crosses the
submission in the vessel's own writable cell, because no register survives one.

**The interpreter is no longer an oracle for this vessel.** A host symbol
exists only in a native object, so the VM refuses the call. It previously asked
the dynamic loader for a library called `host`, which failed here but would have
bound an unrelated symbol on a machine that had one; it now refuses by name.
The build stage requires that exact refusal, so the limit is stated rather than
implied -- but it does mean the independent semantic route ADR 0049 had is not
available to a vessel that reaches its host.

## Three things the first boot found that reading could not

**The user window is high, and that broke the code model.** `address_space`
refuses a user mapping whose PML4 index is zero, because the first page-table
entry is the kernel's own branch. So the vessel cannot live at `0x400000` like
ADR 0049's probe, which uses the separate `arena_profile` machinery; it has to
go in the rite fixtures' window above 512 GiB. The Ny object then would not
link at all:

```
relocation truncated to fit: R_X86_64_32 against `.rodata.str1.1'
```

Every address the small code model materializes is a 32-bit absolute. Fixed
upstream with `nilassembler --code-model large`, which is independent of
`--freestanding` and `--no-red-zone` because where an object is placed is a
separate question from how it is entered. Large-model output arrives in
`.ltext` and `.lrodata`, which the linker script now names --
`--orphan-handling=error` is what reported that, which is the argument for
keeping it on.

**A weak declaration cannot be called from high memory.** The runtime archive
linked high except for one relocation: `R_X86_64_PLT32 against undefined symbol
__ny_host_fail`. A weak *declaration* leaves an undefined symbol whose call the
linker resolves to address zero, and that displacement does not fit in 32 bits
from up there -- the link fails over a symbol nobody intended to use. It is now
a weak *definition* that does nothing, which a host's strong definition
replaces as before. The null check went with it, and the archive now imports
nothing at all.

**Guard pages are the gaps, not the neighbours.** The first boot attempt failed
`ny_vessel_guard_mapping_present` because two of the addresses I required to be
absent were the adjacent mappings themselves. The layout's adjacency is now a
compile-time assertion in the fixture, so the guard list cannot drift away from
the map it is guarding.

## The vessel hears something (same day, second slice)

The first slice made the boundary one-way: Ny could speak, and everything it
spoke it had composed out of its own literals. This one makes it two-way. Ny
asks for a line, the kernel delivers one, and Ny composes a third message
around text it could not have contained.

**The toolchain already supported it; the question was whether it worked.**
`@interop("host")` admits `i64`, `f64` and `text` in both directions, so a
host symbol returning `text` -- a pointer to NUL-terminated bytes -- is inside
the profile, and a zero-argument one is too. That is a statement about the
compiler source, not about the SDK generation this vessel is pinned to, so it
was measured before anything was designed around it. A probe built with the
*pinned* tools, linked against the pinned archive and run against a C host:

    returned           29
    emitted            'ny read: a line from the host'
    composed_in_region 1
    heap_used_at_emit  30
    heap_used_after    0

Ny received the host's text, concatenated it with its own literal, and the
pointer it handed back was inside the region the host installed -- 30 bytes
used for 29 of content and its terminator, reset to 0 by the time the context
ended. Everything after this was built on a measurement rather than on the
backend's `pushValue(result, "text")`.

**The text path was gated to one fixture, and only two items of it.** The
operation that actually delivers text is `session_input_read`, and every other
fixture in this kernel -- `rite_test`, `shell_vessel_test`, the three workspace
runtimes -- only ever gets `would_block` from `input_read`. The first guess was
that reaching a real delivery meant widening the ~25 `cfg(any(test, feature =
"input_wait_test"))` attributes in `rite.rs`, which would have been a sprawling
change to the rite core for one fixture's benefit. It is not: the call envelope
validation, the request decoding and the delivery encoder are all
unconditional. Only `session_input_delivery_reply` and
`finish_session_input_delivery` are gated, and adding a fixture to a gate's
list is this codebase's existing idiom. The slice cost two lines there. The
wait/readiness/pending machinery stays gated where it was, which is the honest
boundary: this fixture does not park.

**The reply had to be staged where it is built, not where it is returned.** A
console write is acknowledged with a bare 40-byte header, and the gate's return
path writes that itself. A delivery is 320 bytes whose body must be in the
mailbox *before* the rite is settled and whose validity marker must be written
*after*, so letting the generic path write a header-only reply afterwards would
have destroyed both halves. The dispatcher gained a third outcome --
`published_reply` -- meaning the mailbox already holds the answer; it counts the
copy and returns without writing one.

**What the boot shows.** Five rites, two operations:

    kernel.ny_vessel.ready             grant 39, operations 2, image_bytes 3555
    kernel.ny_vessel.console.committed sequence 1, 38 bytes, from_region true,  carries_delivery false
    kernel.ny_vessel.console.committed sequence 2, 23 bytes, from_region false, carries_delivery false
    kernel.ny_vessel.input.delivered   sequence 3, 11 bytes, human_text, waited false, read_by ny
    kernel.ny_vessel.console.committed sequence 4, 21 bytes, from_region true,  carries_delivery true
    kernel.ny_vessel.exited            sequence 5, ok, code 42, returned_from_ny true
    kernel.ny_vessel.released          65114/65114 frames, 0 owned, 0 active
    kernel.ny_vessel.passed            calls 5, commits 5, console_writes 3, deliveries 1

The 21 bytes at sequence 4 are `ny heard: say it back`: ten bytes Ny had and
eleven it was given. The vessel's image cannot contain those eleven, so the
count alone is evidence they arrived -- but the stronger evidence is in the
region. After the vessel exits the kernel searches the region page and requires
three things: the first message at the base, as the first allocation; the third
message present somewhere, which means the delivered line reached Ny's own
variables and was composed there; and the second message **absent**, because it
is a literal quoted straight out of the image and never allocated. That last
check is what makes `from_region` in the transcript a measurement rather than a
label, and it distinguishes composing from quoting in both directions.

**`waited` is false and says so.** The vessel's request sets `wait_if_empty`,
which is the request a real vessel would send, but this fixture is the only
source of input and its line is ready before the vessel asks, so no wait is
ever installed. The transcript states that as a required field rather than
leaving a reader to assume a blocking path was exercised. Nothing here shows
that a Ny vessel survives being descheduled inside its own control flow; that
needs a second task to publish the input, which is `input_wait_test`'s
machinery, and it is the next slice.

**The one-page budget is now a real constraint.** The RX image is 3,555 of
4,096 bytes. The input stub's envelope and body validation is most of what it
cost. A third operation will fit; a fourth may not, and the fixture maps
exactly one code page by design.

**Smaller things worth keeping.** The stub validates the delivery's length
against the 256-byte capacity rather than against the 11 bytes this fixture
sends, so it is a binding rather than a transcript transcription -- what is
pinned to this fixture's exact text is the kernel side. The third message is
written as one literal in the fixture for readability and tied to its two
halves by a `const` assertion, so a delivery changed without changing what the
vessel is expected to say fails to compile rather than failing in QEMU.

## What happens when the region runs out, measured

The first slice's record said a vessel that concatenates in a loop "exhausts
the region and stops". That was a guess dressed as a statement, so it was
measured. A Ny program that concatenates two hundred times, linked against the
pinned archive and run against a C host:

    __ny_host_fail reached: 'NIL native runtime error: allocation failed during string concat'
    heap_used at failure: 3999
    exit=17

and the same object with the host's strong definition removed, so the archive's
weak no-op stands:

    exit=124    (timed out)

So the sequence is: `nyrt_alloc` returns null, the runtime calls `nyrt_fail`,
`nyrt_fail` calls `__ny_host_fail`, and when that is the archive's weak no-op
it returns and `nyrt_fail` spins in `for (;;)` forever. In this kernel that is
a user-mode task looping with no rite ever submitted. The vessel does not
fault, does not exit, and submits nothing, so the dispatcher sees nothing at
all; the only thing that would eventually notice is the smoke timeout.

The way out is already there and is the reason `__ny_host_fail` became a weak
*definition* rather than a weak declaration in the first slice: a strong
definition in the binding replaces it, is reached with the message, and can
submit a `console_write` of that message followed by a `task_exit` with a
status that says Ny failed. Then a vessel's failure is a rite like anything
else, and the kernel can state it.

Two things made proving it awkward, and both were decided rather than worked
around:

1. **One run cannot both succeed and fail.** Showing the failure path needs a
   vessel that fails, and the evidence already banked -- three writes, the
   delivery, exit 42 from Ny's own return value -- needs one that does not.
   The fixture now enters the same image twice, and the host asks for the run
   it wants through a cell in the vessel's state page: written before entry
   exactly as the region is installed, read back by a host symbol that is not
   a rite at all. A second payload would have duplicated the whole stage; two
   entries of one image also show that a Ny object can be entered twice from
   fresh state, which is worth having on its own.
2. **The failure message is the runtime's, not ours.** It is English prose
   from NIL with a trailing newline. The binding writes it as it arrived,
   minus the framing the console will not admit, and the kernel checks it by
   shape rather than by bytes: printable, bounded by the cell it was copied
   into, and containing `allocation failed`. The oracle pins its length as a
   tripwire, which is deliberate -- a reworded message is a change to what a
   vessel's failure looks like from outside, and that should fail here and be
   looked at rather than pass unnoticed.

## What the failing run shows

    kernel.ny_vessel.task.entered      run "failing", mappings 3, guards absent
    kernel.ny_vessel.failure.reported  sequence 1, 64 bytes, composed_by runtime,
                                       issued_by binding, from_region false,
                                       names_the_cause true
    kernel.ny_vessel.exited            sequence 2, ok, code 70, returned_from_ny false
    kernel.ny_vessel.released          65114/65114 frames, 0 owned, 0 active

Four things in there are the point. The message arrived **as a rite**, which
without the binding's hook it could not have done at all. `composed_by` is
`runtime`, not `ny`: these are the only bytes in the whole transcript the
vessel did not write. `returned_from_ny` is false, because Ny never returned --
the status is the binding's choice, not a value Ny picked, and the transcript
does not pretend otherwise. And the release accounting is identical to the
working run's: a vessel that failed inside Ny still owes its frames back, and
the region it exhausted is still wiped.

The region is read afterwards for the opposite reason to the working run: it
must still begin with `01234567890123456789`, this run's first two
allocations. Without that a run that failed for any other cause would look
exactly the same from outside.

**The failure path cost no new resume address.** `__ny_host_fail` calls the
console stub the other writes already use and then jumps into the entry's own
exit submission, so the header still states three code addresses rather than
five, and the dispatcher validates the same ones. Routing a second copy of
the submission sequence through the binding would have been easier to write
and worse to trust.

**What it did cost was a page.** The image went from 3,555 to 4,113 bytes, so
the code is now a two-page extent. The pair is mapped with `map_owned_extent`,
so the mapping count is unchanged at three and the circle's budget is
untouched; the writable extent, the region, the mailbox and the stack all
moved up by one page. That is the first time the vessel's map changed since it
was laid out.

**And it exposed an ABI gap worth naming.** The rite ABI has no way to say
"the language runtime failed". The binding picks exit code 70; the kernel
believes it; nothing in the reply or the task's final state distinguishes that
from a vessel that chose to exit 70. A supervisor deciding whether to restart
something would want to tell those apart, and a convention held one vessel at
a time is not where that belongs.

## Open, in the order they probably matter

1. ~~**Ny cannot issue a rite itself.**~~ Done, above.
2. ~~**Only `console_write` is reachable from Ny.**~~ Two operations now are.
   `display_update` and the scheduler gate are still not declared and so are
   not imported, and the plain `input_read` operation remains the one no
   fixture in this kernel has ever seen succeed.
3. **No rite here can actually block.** The vessel asks to wait and is always
   answered at once, because this fixture is its only source of input. The
   open question is whether a Ny vessel survives being descheduled inside
   `main` -- its stack and the region have to be exactly as it left them when
   it resumes, and nothing has yet put that to the test. It needs a second
   task to publish the input, which is `input_wait_test`'s machinery, and it
   is the most interesting thing left in this direction.
4. ~~**A vessel that exhausts its region hangs, and nothing here would
   know.**~~ Closed: the binding defines `__ny_host_fail`, and the failing run
   proves it in the running system rather than on a Linux host. What is left
   is the ordinary limit -- a long-running vessel still has no way to reclaim
   the region short of ending the context.
5. **A Ny failure is an exit code, not a status.** Nothing distinguishes a
   vessel whose runtime gave up from one that chose to exit 70. That wants a
   place in the ABI rather than a convention held one vessel at a time, and it
   is the first thing this slice found that cannot be fixed inside Anomalous.
6. **Text comparison is unusable in a freestanding object.** `==` on text
   emits a direct `strcmp` at three sites in NIL's backend, so it would fail
   this vessel's own import check. The concat path was fixed exactly this way
   once already -- it used to import `strlen`, `calloc` and `memcpy` -- so
   this is the same defect in the comparison path, and it wants an `nyrt_`
   hook beside the others. It is why the failing run is selected by a cell
   rather than by the vessel recognising a word in the line it was delivered.
7. **The code extent has room again, and the region is the tighter of the
   two.** 4,113 of 8,192 bytes of code; the region is one page and the working
   run spends 30 of its 4,096 bytes.
8. **Nothing checks the host signature.** The toolchain cannot see the
   definition, so the declarations in `vessel.ny` and the stubs in `entry.S`
   are reviewed together; a mismatch would be caught by neither side.
3. ~~**The kernel harness.**~~ Done. For the record, the design as planned:
   `ny_vessel_test.rs` modelled on `rite_test.rs`, with `ny_vessel_entry.S`
   for the gate entry and the enter/resume pair, mapping the three extents
   above, dispatching two rites through `rite_transport`, and checking the
   console bytes, the exit status and release accounting. The payload arrives
   through the wrapper symbols rather than being assembled into the kernel,
   because a Ny vessel's code is produced by the toolchain. Wiring: a
   `ny_vessel_test` cargo feature, `mod`/`global_asm!`/`run_ny_vessel_test` in
   `x86_64.rs` and additions to the mutual-exclusion lists there,
   `ny_vessel_smoke.rs` with its QEMU run and transcript oracle, and
   `ny-vessel-image`/`ny-vessel-smoke` targets. `rite_transport.rs` is already shared by eight
   vessel fixtures, so a ninth is the ordinary unit of work here: map the RX
   page, the state page and the region, enter the task, dispatch two rites,
   and check the console bytes and exit status against the measured values
   above. The payload has to be built by an xtask stage, the way
   `ny_object_artifact.rs` does it, because a Ny vessel's code is produced by
   the toolchain rather than assembled into the kernel.
2. **An ADR.** This is a new profile, not a variation of ADR 0049's: the image
   contains an allocator, which that ADR's admission explicitly excludes, and
   the region-as-channel contract is new. ADR 0049's fixture should be left
   qualified and untouched; this is a sibling slice with its own record.
3. **ADR 0049's own pin is stale.** It pins generation `c5416212…` and the
   tool names `nycompiler`/`nyassembler`/`nyinterpreter`, none of which exist
   in a current SDK, and its import list expects `calloc`/`memcpy`/`strlen`,
   which a freestanding object no longer has. Re-pinning it is a separate
   decision from this slice, and worth taking deliberately: the fixture is the
   recorded evidence for a qualification.
4. **Ny cannot yet call a rite itself.** The binding here runs before and
   after `main`, so a vessel cannot do I/O from inside its own control flow.
   That needs a way for Ny to call a host-provided symbol -- the capability,
   not the bindings -- and is a NIL-side question.
