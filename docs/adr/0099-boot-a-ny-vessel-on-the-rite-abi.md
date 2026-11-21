# ADR 0099: Boot a Ny vessel on the rite ABI

- Status: Proposed
- Date: 2026-11-09
- Builds on: ADRs 0016, 0019 and 0049

## Context

[ADR 0049](0049-qualify-a-bounded-freestanding-ny-object.md) qualified a bounded
freestanding Ny object, and recorded what consuming one cost. The SDK emitted a
Linux host-targeted object plus a generated C `main(int, char **)` wrapper, so a
narrow assembly shim had to call that wrapper and implement its *observed*
ABI-version, context-begin, context-end and failure hooks, because nothing
published them. The compiler's unconditional string helper imported `strlen`,
`calloc` and `memcpy` even for scalar source, satisfied here with fatal trap
stubs. The ordinary runtime assumed hosted allocation, OS services and TLS. Ny
had no red-zone-disabling option.

Every one of those was a property of what the toolchain emitted, and all of them
are now answered upstream. An object emitted with `--freestanding` is entered
through `__ny_entry`, defines no `main`, and imports five native runtime hooks --
plus three more only if it allocates. It imports no C library symbol under any
configuration. The SDK ships `libnilruntime_freestanding.a`, which implements
those hooks with no libc, no kernel and no heap of its own, and a generation
records it by hash.

That removes the reason for a reverse-engineered shim, for trap stubs, and for
an allocator we would have had to write. It does not by itself show that a
vessel boots or completes a rite, which is what this slice is for.

## Decision

Add the opt-in `ny_vessel_test` fixture: one Ny vessel, booted by this kernel
twice from the same image, completing ten rites across three operations
through the ordinary dispatcher -- six on the run that works, two on a second
task that runs while that one is parked, and two on the run that fails inside
Ny on purpose.

The vessel is `vessels/anomalous-ny-vessel`. **Ny issues the rites itself.**
The console write is a symbol this vessel's Anomalous half defines and its Ny
half declares with `@interop("host")`, so it is called from inside Ny's own
control flow. The vessel composes a message by concatenation rather than naming
a constant, because composition is what needs the allocator; writes it; reads
the count the rite returned; and writes a second line only because that count
was the one it sent. Then it returns 42.

Then it reads. A second host symbol submits one `session_input_read`, and the
vessel composes a third message around the line that comes back and writes
that. This is the first rite here whose reply carries a payload and the first
to bring the vessel data it could not have contained, so it is the first thing
in this profile that makes the boundary two-way: text from the kernel, into
Ny's own variables, composed in the region, and back out through a write.

It also gives up the processor, and something else takes it. Between reading
the line and composing its answer, Ny submits one `task_yield`. Answering that
rite is where a second task runs: its own circle, its own address space, its
own mailbox and its own privilege-transition stack, entered at the image's
peer door while the vessel sits parked in the gate. The answer is then
composed from a pointer Ny was holding before the yield, so the bytes found in
the region afterwards are what shows that Ny's stack frame, that pointer and
the bump allocator all survived another task running. The vessel checks it
too: the stub returns how many yields have completed and Ny refuses with its
own status if that is not one.

**The second task is this image at a different door.** It borrows the console
stub and the entry's exit submission, because a second task makes its point by
running at all -- in its own address space, through its own mailbox -- not by
running something the vessel does not already contain. It never enters
`__ny_entry`, and the transcript records that as `entered_ny` false. Its grant
is `console_write` and `task_exit` and nothing else, so a peer that tried to
read input or yield would be refused by its own channel.

**The hand-off is this fixture's, not the scheduler's.** It enters the second
task from inside the parked vessel's dispatch, which means saving and clearing
the three single-use continuation cells and the reentry latch for the inner
entry and restoring them afterwards. The latch is released across exactly that
window: it means "a dispatch is in progress on this context", and the inner
task legitimately begins its own, during which the vessel cannot run.
`scheduler::preempt_current` is ungated and would serve, but reaching it needs
per-task privilege stacks, a gate entry that returns the frame to resume and a
scheduler queue -- a rebuild of this fixture's control flow. The transcript
states `scheduler_preempted` as false so this cannot be read as evidence of
scheduler-driven preemption.

One bug that cost a boot is worth recording, because the next nested entry
anywhere will meet it. `descriptors::wipe_privilege_stack` zeroes the single
static transition stack, and its own contract says it may be called only after
the caller has abandoned that stack and restored the original kernel stack.
That holds for a run entered from the bootstrap stack and not for a nested
one: the parked vessel's saved frame and the dispatch's own return addresses
are on it, so wiping produced an invalid opcode at kernel instruction pointer
zero. A nested entry does not wipe, and nothing is left unscrubbed -- the
second task's transition stack is its own lease and is cleared when that lease
is released.

And it can fail. The shipped runtime defines `__ny_host_fail` as a weak
no-op and spins in `nyrt_fail` once it returns, so a Ny failure in a vessel is
a user task looping with no rite submitted: it does not fault, does not exit,
and says nothing, and only a timeout would notice. The binding now defines
that hook, so a failure leaves by the same door as a success -- the runtime's
message is spoken as a `console_write` and the task exits with a status saying
Ny failed rather than a value Ny chose. The hook reuses the console stub and
jumps into the entry's own exit submission, so it adds no third resume address
and no second copy of the ABI.

Proving it needs a vessel that fails, and the evidence the working run banks
needs one that does not, so the image is entered twice and the host says which
run it wants through a cell in the vessel's own state page -- written before
entry exactly as the region is installed, and read back by a host symbol that
is not a rite at all. The failing run exhausts the region by concatenating,
which is the one failure a vessel can reach deliberately. Entering the same
image twice from fresh state is worth having on its own account.

The surrounding entry installs the region, calls `__ny_entry`, and submits
`task_exit` with what Ny returned. It no longer touches the messages at all.

Rite bindings stay here, under ADR 0019, because this ABI is ours and is
explicitly unstable pre-alpha; nothing of it enters the Ny standard library.
What the toolchain supplies is the capability to reach a symbol the consumer
defines -- the declared Ny function *is* an external symbol of that name,
called with the ordinary C convention -- and nothing about this gate, this
mailbox or this vector is visible to Ny.

**The region is the channel.** A Ny object publishes `__ny_entry` and nothing
else: its globals are absent from the symbol table, and its interop surface
admits only a closed allow-list of versioned libraries, so it can neither
export data nor call one of our symbols. The only things crossing the boundary
are the entry's return value and the hooks it calls. So the kernel installs a
region with `nyrt_heap_init`, Ny allocates its message into that region, and
the vessel's Anomalous half reads the message back out of memory the kernel
owns. Ny never learns an address. Ending the Ny context resets the allocation
pointer without scrubbing the region, which is what makes the read valid, and
the shipped runtime states that contract.

**Memory profile.** Three mappings, because a circle admits three: one
user-RX two-page code extent; one user-RW, NX three-page extent holding the
vessel's state, its region and its mailbox; and one user-RW, NX stack page.
The code was one page until the vessel learned to fail, and the second page
costs no mapping because the pair is mapped as a single extent. The window
base, the page above the extent and the page above the stack are required
absent. The mailbox sharing the vessel's own writable extent is not a
weakening: the dispatcher reads it through the kernel's physical alias and
copies admitted bodies rather than trusting it, and a vessel may write anywhere
in its own pages regardless.

The vessel is linked into the same user window the rite fixtures use, above the
512 GiB boundary, because the first page-table entry is this kernel's own
branch and a user mapping there is refused. Reaching it requires the object to
be emitted for the large code model; the small model materializes every address
as a 32-bit absolute and cannot be linked there at all. `--code-model large` is
a new, independent toolchain option, and the runtime archive needs no
equivalent because it reaches its own statics relative to the instruction
pointer.

**The payload is built, not assembled here.** A Ny vessel's code comes from the
toolchain, so `cargo xtask ny-vessel-inspect` compiles, assembles, links and
inspects it, then carries the bytes into the kernel link as a relocation-free
opaque wrapper with a header stating the addresses the dispatcher validates
each gate return against. It refuses a generation whose manifest, compiler,
assembler, interpreter or freestanding archive hash differs from the pin -- the
archive separately, so a generation that changed only that file is still
rejected; an object importing anything but the eight runtime-ABI names and the
three symbols the binding defines; a link that acquires an undefined symbol or
an unplaced section; and an image that does not begin at its page base, does
not fit its code extent, places its failure or return labels outside that
extent, puts its mode cell anywhere but its own state page, moves its writable
layout, or lets the region reach the mailbox. The image occupies 4,113 of its
8,192 bytes.

The vessel's grant is the shell minimum plus `session_input_read` and
`task_yield` -- `0b101111` -- which is what its three operations require and
no more. Reaching the delivery
widened two `cfg` gates in the rite core, `session_input_delivery_reply` and
`finish_session_input_delivery`, from the one fixture that first needed them to
include this one. The blocking machinery beside them -- the wait, readiness and
pending types -- stays gated where it was, because nothing here parks.

This fixture owns interrupt vector `0xf3` exclusively while it is enabled;
every other rite feature is excluded from selection, as those features already
exclude one another.

## Validation and limits

Qualified on 2026-11-09 under TCG. The vessel entered user mode with three
mappings and absent guards, committed sequence 1 as 38 bytes of UTF-8 through
`console_write` with status `ok`, committed sequence 2 as a further 23 bytes --
which it sends only because the first rite returned the count it had passed --
was delivered 11 bytes of `human_text` at sequence 3, yielded at sequence 4,
committed sequence 5 as the 21 bytes it composed around the delivered line
*after* the second task had run, and exited at sequence 6 with status `ok` and
code 42. The second task committed 33 bytes of its own at its sequence 1 and
exited at its sequence 2 with code 55, in its own address space, while that
vessel was parked.

The same image was then entered a second time with its mode cell set. It
exhausted its region, the runtime's failure hook reached the binding, and the
binding committed the runtime's 64-byte message at sequence 1 and exited at
sequence 2 with code 70 -- `returned_from_ny` false, because Ny never
returned. Release accounting restored 65,114 of 65,114 free frames after each
run with no owned frames, circles, tasks or mappings left, the channel
retired, the region wiped and the privilege stack wiped: a vessel that failed
inside Ny still owes its frames back.

The transcript oracle requires that exact sequence of twenty records and those
exact values. The vessel's own message lengths and its exit code are the ones
measured by running the vessel's pre-gate sequence against the same object and
the same archive on the build host. The failure message's bytes are not
pinned: they are English prose owned upstream, and this repository is not
where their wording is decided. What is required of them is that they are
printable, that they fit the cell the binding copies them into, and that they
name the cause -- the kernel looks for `allocation failed` and nothing more
specific. Its length is pinned as a tripwire, so a reworded message fails here
and is looked at rather than passing unnoticed.

The kernel additionally reads the region back after the vessel exits and makes
three checks there. The first message must be present, NUL-terminated, as the
vessel's first allocation. The third message must be present somewhere in the
region and contains the delivered line, which the vessel's image does not and
cannot contain -- that is this slice's claim, stated as bytes in a page rather
than as a count the vessel reported. And the second message must be *absent*:
it is a literal quoted out of the image, so finding it in the region would mean
the vessel composed where it was expected to quote, and the `from_region` field
of the transcript would be wrong. The fixture also ties the third message to
its two halves at compile time, so a delivery changed without changing what the
vessel is expected to say fails to build.

On the failing run the region is read for the opposite reason: it must still
begin with that run's first two allocations, so a run that failed for some
other cause could not pass as a region the vessel exhausted by concatenating.

This establishes one Ny source and object, one binding, two entries of that
image, two operations reached from inside Ny, one reply payload carried into
Ny, and one Ny failure reaching the kernel as a rite. It does not
qualify the hosted Ny runtime, a general Anomalous compiler target, an SDK
release, Ny strings beyond two concatenations, TLS, AVX/XSAVE, unwinding, an
application service ABI, Antimatter, HAG, or privileged Ny migration. Existing
verified Rust remains authoritative under ADR 0016.

Seven limits are worth stating precisely, because each is a next slice rather
than an omission:

1. **Two operations are reachable from Ny, not the whole ABI.** The vessel
   declares `console_write` and `session_input_read`; `input_read`,
   `display_update` and the scheduler gate are not declared and so are not
   imported. Each is a line of binding when a vessel needs it, and the import
   surface states which exist.
2. **Nothing blocks, and the hand-off is not the scheduler's.** Two limits
   that are easy to confuse, kept apart by the transcript. The input request
   sets `wait_if_empty`, which is what a vessel should send, but this fixture
   is the only source of input and its line is ready before the vessel asks,
   so no wait is ever installed -- `waited` is required to be `false`. And a
   second task does get the processor, but this fixture enters it directly
   rather than the scheduler selecting it: `scheduler_preempted` is required
   to be `false`.

   So what is established is the part specific to a Ny vessel -- its state
   survives another task running in its own address space while it is parked
   mid-`main`. What is not established is that this kernel's scheduler can
   park and resume such a task, which is a different claim and belongs with
   the scheduler rather than with the vessel. `scheduler_test` proves it for
   its own task shape, and reaching it from here needs per-task privilege
   stacks for both tasks, a gate entry that returns the frame to resume and a
   scheduler queue. The primitives are not the obstacle: `preempt_current`,
   `scheduler_table::new`, `admit` and `task_yield`'s completion are all
   ungated, and the four privilege-stack items a second task needs are now
   reachable from this feature.
3. **The region is never reclaimed inside a context.** A vessel that
   concatenates in a loop runs out, which is correct for this contract and
   wrong for a long-running one. The failing run is that case on purpose, so
   what it does is now measured rather than assumed: `nyrt_alloc` returns null
   at 3,999 of 4,096 bytes used and the runtime calls `nyrt_fail`. What is no
   longer true is that this hangs the vessel -- the binding's own
   `__ny_host_fail` turns it into two rites. Without that definition the
   archive's weak no-op returns and `nyrt_fail` spins in `for (;;)` forever,
   which is what a consumer who does not define the hook will get. The
   remaining limit is the one a long-running vessel will actually hit: there
   is no way to reclaim the region short of ending the context, and a vessel
   that wants to keep allocating has nowhere to go.
4. **The interpreter cannot run this vessel.** A host symbol exists only in a
   native object, so the VM refuses the call by name. The build stage requires
   that refusal rather than leaving it implied, but it means the independent
   semantic oracle ADR 0049 had is not available to a vessel that reaches its
   host.
5. **Nothing checks the host signature.** The toolchain cannot see the
   definition, so its type profile is narrow -- `i64`, `f64` and `text`, where
   `text` is a pointer to NUL-terminated bytes -- and a mismatch between the
   declaration and this file's stub would not be caught by either side. The
   stub and the declaration are reviewed together for that reason.
6. **A Ny failure is an exit code, not a status.** The rite ABI has no way to
   say that the language runtime failed. The binding picks 70 and this kernel
   believes it, so nothing in the reply, the exit payload or the task's final
   state distinguishes a vessel whose runtime gave up from a vessel that chose
   to exit 70. A supervisor deciding whether to restart something would want
   to tell those apart. If that distinction matters it belongs in the ABI
   rather than in a convention held one vessel at a time.
7. **ADR 0049's fixture is untouched and its own pin is stale** against a
   current SDK. It remains the recorded evidence for its qualification;
   re-pinning it is a separate decision.

The [Ny vessel log](../../notebook/Development/2026-11-09%20Ny%20Vessel%20on%20the%20Rite%20ABI%20Log.md)
records the implementation evidence and what each measurement does and does not
show.
