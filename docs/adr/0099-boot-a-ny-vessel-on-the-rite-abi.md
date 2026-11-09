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
completing two rites through the ordinary dispatcher.

The vessel is `vessels/anomalous-ny-vessel`. Its Ny half composes a message by
concatenation rather than naming a constant, because composition is what needs
the allocator, and returns 42. Its Anomalous half is the whole of the binding
and is deliberately small: install the region, call `__ny_entry`, take the
message's length and copy it into the mailbox payload through the runtime
rather than libc, submit one `console_write`, then `task_exit` carrying what Ny
returned. Rite bindings stay here, under ADR 0019, because this ABI is ours and
is explicitly unstable pre-alpha; nothing of it enters the Ny standard library.

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
rejected; an object importing anything but the eight runtime-ABI names; a link
that acquires an undefined symbol or an unplaced section; and an image that
does not begin at its page base, does not fit one page, places its failure or
return labels outside that page, moves its writable layout, or lets the region
reach the mailbox. The interpreter runs the same source as an independent
semantic oracle whose exit status must be the 42 Ny returns.

This fixture owns interrupt vector `0xf3` exclusively while it is enabled;
every other rite feature is excluded from selection, as those features already
exclude one another.

## Validation and limits

Qualified on 2026-11-09 under TCG. The vessel entered user mode with three
mappings and absent guards, committed sequence 1 as 38 bytes of UTF-8 through
`console_write` with status `ok`, and exited at sequence 2 with status `ok` and
code 42. Release accounting restored 65,116 of 65,116 free frames with no owned
frames, circles, tasks or mappings left, the channel retired, the region wiped
and the privilege stack wiped. The transcript oracle requires that exact
sequence of thirteen records and those exact values; the message's length and
the exit code are the ones measured by running the vessel's pre-gate sequence
against the same object and the same archive on the build host.

The kernel additionally reads the region back after the vessel exits and
requires the message to be there, NUL-terminated, as the vessel's first
allocation. A vessel that had sent bytes it did not compose fails there.

This establishes one Ny source, object, binding and boot. It does not qualify
the hosted Ny runtime, a general Anomalous compiler target, an SDK release, Ny
strings beyond one concatenation, TLS, AVX/XSAVE, unwinding, an application
service ABI, Antimatter, HAG, or privileged Ny migration. Existing verified
Rust remains authoritative under ADR 0016.

Three limits are worth stating precisely, because each is a next slice rather
than an omission:

1. **Ny cannot issue a rite itself.** The binding runs before and after Ny's
   `main`, so a vessel cannot perform I/O from inside its own control flow.
   That needs a way for Ny to call a host-provided symbol -- the capability,
   not the bindings -- which is a toolchain question.
2. **The region is never reclaimed inside a context.** A vessel that
   concatenates in a loop exhausts it and stops. That is correct for this
   contract and wrong for a long-running vessel.
3. **ADR 0049's fixture is untouched and its own pin is stale** against a
   current SDK. It remains the recorded evidence for its qualification;
   re-pinning it is a separate decision.

The [Ny vessel log](../../notebook/Development/2026-11-09%20Ny%20Vessel%20on%20the%20Rite%20ABI%20Log.md)
records the implementation evidence and what each measurement does and does not
show.
