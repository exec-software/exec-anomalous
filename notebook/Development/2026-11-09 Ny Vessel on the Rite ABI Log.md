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

## Three things the boot found that reading could not

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

## Open, in the order they probably matter

1. **Ny cannot issue a rite itself.** The binding runs before and after Ny's
   `main`, so a vessel cannot do I/O from inside its own control flow. That
   needs a way for Ny to call a host-provided symbol -- the capability, not the
   bindings -- and is a toolchain question. It is the obvious next slice.
2. **The region is never reclaimed inside a context**, so a vessel that
   concatenates in a loop exhausts it and stops. Correct for this contract,
   wrong for a long-running vessel, and it wants a real vessel to ask it.
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
