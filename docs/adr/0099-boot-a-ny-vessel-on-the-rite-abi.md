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

Add the opt-in `ny_vessel_test` fixture: one Ny vessel, booted by this kernel,
completing five rites across two operations through the ordinary dispatcher.

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
user-RX code page; one user-RW, NX three-page extent holding the vessel's
state, its region and its mailbox; and one user-RW, NX stack page. The window
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
two symbols the binding defines; a link that acquires an undefined symbol or an
unplaced section; and an image that does not begin at its page base, does not
fit one page, places its failure or return labels outside that page, moves its
writable layout, or lets the region reach the mailbox. The image occupies 3,555
of its 4,096 bytes, so the one-page budget is a real constraint on what else
this vessel can be asked to do.

The vessel's grant is the shell minimum plus `session_input_read` -- `0b100111`
-- which is what its two operations require and no more. Reaching the delivery
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
was delivered 11 bytes of `human_text` at sequence 3, committed sequence 4 as
the 21 bytes it composed around them, and exited at sequence 5 with status `ok`
and code 42. Release accounting restored 65,114 of 65,114 free frames with no
owned frames, circles, tasks or mappings left, the channel retired, the region
wiped and the privilege stack wiped. The transcript oracle requires that exact
sequence of sixteen records and those exact values; the message lengths and the
exit code are the ones measured by running the vessel's pre-gate sequence
against the same object and the same archive on the build host.

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

This establishes one Ny source, object, binding and boot, two operations
reached from inside Ny, and one reply payload carried into Ny. It does not
qualify the hosted Ny runtime, a general Anomalous compiler target, an SDK
release, Ny strings beyond two concatenations, TLS, AVX/XSAVE, unwinding, an
application service ABI, Antimatter, HAG, or privileged Ny migration. Existing
verified Rust remains authoritative under ADR 0016.

Six limits are worth stating precisely, because each is a next slice rather
than an omission:

1. **Two operations are reachable from Ny, not the whole ABI.** The vessel
   declares `console_write` and `session_input_read`; `input_read`,
   `display_update` and the scheduler gate are not declared and so are not
   imported. Each is a line of binding when a vessel needs it, and the import
   surface states which exist.
2. **Nothing blocks.** The vessel's request sets `wait_if_empty`, which is
   what a vessel should send, but this fixture is the only source of input and
   its line is ready before the vessel asks, so no wait is ever installed. The
   transcript says so in as many words -- `waited` is required to be `false`.
   A rite that actually parks the vessel and resumes it needs a second task to
   publish the input, which is the `input_wait_test` fixture's machinery and a
   separate slice. Until that slice exists, nothing here shows that a Ny
   vessel survives being descheduled inside its own control flow.
3. **The region is never reclaimed inside a context, and exhausting it hangs
   the vessel.** A vessel that concatenates in a loop runs out, which is
   correct for this contract and wrong for a long-running one. What happens
   then was measured rather than assumed: `nyrt_alloc` returns null at 3,999
   of 4,096 bytes used, the runtime calls `nyrt_fail` with `allocation failed
   during string concat`, and because the shipped archive defines
   `__ny_host_fail` as a weak no-op, `nyrt_fail` then spins forever in user
   mode. This kernel would learn nothing; only the smoke timeout would. A
   strong `__ny_host_fail` defined by the binding is reached cleanly, so the
   way out exists and this vessel does not yet take it. That is a hole rather
   than a missing test, and closing it is the next slice.
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
6. **ADR 0049's fixture is untouched and its own pin is stale** against a
   current SDK. It remains the recorded evidence for its qualification;
   re-pinning it is a separate decision.

The [Ny vessel log](../../notebook/Development/2026-11-09%20Ny%20Vessel%20on%20the%20Rite%20ABI%20Log.md)
records the implementation evidence and what each measurement does and does not
show.
