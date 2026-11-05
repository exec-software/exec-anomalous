# Handoff — program launch, arena profile, and the ELF co-ordination

Written 2026-10-15, late. Two agents are in this repository: Codex is
qualifying ADR 0091 (allocator-lifetime) on the ELF/Linux side; this note is
the NIL-runtime/ABI side.

## One-line state

`program_descriptor` now crosses the launch boundary, the vessel that reads it
exists, and the mapping-core change that would let the kernel launch it is
written and type-checked but **uncommitted, pending Codex's review**.

## What landed (pushed, main)

| Commit | What |
| --- | --- |
| `5f2a567` | Linker fix: the vessel could not link at all |
| `ec573be` | AGENTS.md note on the workspace-build trap |
| `a8b4ce8` | Formatting: `make check` step one had been failing |
| `5a0c9b1` | `ANPG` wire form for `program_descriptor` |
| `14e9083` | Strict-clippy fixes in the runtime crate (Codex was blocked) |
| `b85410a` → `b08a16d` | ADR 0094 proposed, then revised after review |
| `3dd03a2` | `vessels/anomalous-native-program-client` |

## Uncommitted: the arena profile patch

In the working tree, 650 lines over `kernel/Cargo.toml`,
`owned_execution.rs`, `prepared_launch.rs`, `physical_access.rs`. A copy is at
`arena-profile.patch` in the session scratchpad, which does **not** survive.
If the tree is clean and these are absent, it was discarded; regenerate from
ADR 0094, which is accurate.

- `arena_profile` kind: `native_application` (8 RX pages, 32 KiB, bases
  `0x804000`/`0x814000`) and `program` (1 RX page, 64 KiB, bases
  `0x804000`/`0x818000`) behind `native_program_test`.
- `arena_mapping_plan` factored out pure and allocation-free, so geometry is
  assertable without an access group, and refusal costs no allocation.
- Golden tests restate every native value independently of the profile, and
  reject twelve non-admitted bases. **Type-checked, never run** — tests were
  outside the agreed allowance.
- `prepared_launch` retains `arena_kind`; the revalidation path and
  `arena_extent` answer to it instead of hardcoding `0x804000 | 0x814000`,
  `expected_code_pages != 8` and `8 * page_size`.
- `prepare_program_arena`: a separate single-task entry point. The
  `task_count == 1` gates are feature-selected in `owned_execution.rs` and at
  both `physical_access.rs` sites (162, 287).

Verified: `cargo check --all-targets` on default plus eight feature
configurations; strict clippy on the five `make check` sets this touches,
including `linux_loader_entry_test`; `cargo fmt` clean.

## The next step, and it is smaller than it looks

A kernel profile attaches by **one `cfg_attr` path arm**, not by new plumbing:

```rust
// native_application_runtime.rs ~line 148
#[cfg_attr(feature = "native_program_test", path = "native_program_profile.rs")]
mod native_runtime_profile;
```

The module name stays, so all thirteen delegation points
(`is_active`, `user_frame_valid`, `timer`, `fault`, `gate`,
`resume_kernel_after_fault`, `resume_input_interrupt`, `run`) are unchanged.
`native_document_profile.rs` and `native_relaunch_profile.rs` are already
alternative backings of that same module — mirror the smallest.

The profile itself must: inspect the payload, `prepare_program_arena`, write
the `ANPG` descriptor at `descriptor_offset` 4096, read it back and `decode`
to confirm, set `frame.destination_index = descriptor_address` and
`frame.source_index = program_startup_size`, admit gate op 0 and op 5, then on
exit read the six-word proof record from arena offset 0 and compare it against
what was granted. The vessel writes: arena length, edition length, granted
services, argument length, console present, edition handle.

The payload reaches the kernel through xtask: `native_runtime_artifact.rs`
emits a `payload.S` with `.incbin` of the vessel's RX bytes and symbols
`x86_64_native_runtime_payload_{start,end}`, assembles it, and links it in. A
program artifact module mirrors that.

Still open: `clippy -D warnings` on `native_program_test` alone reports
`program` never constructed and `prepare_program_arena` never used, because
that profile is the missing consumer. No `make check` line builds the feature,
so the validation set is unaffected. It closes when the profile lands.

## Things that cost time, so they should not cost it twice

**Never insert an item directly above an existing `#[cfg]` attribute.** Done
twice today in two files. The attribute silently re-gates the new item and
un-gates the one it was written for. The second instance only appeared under
`elf_application_test`, which is why all nine configurations get checked, not
the one being worked in.

**A vessel builds only under the project's own rustflags.** `code-model=small`,
`relocation-model=static`, `-Z plt=yes` with
`-Zbuild-std=core,compiler_builtins`. A plain `cargo build -p <vessel>` picks
up `.cargo/config.toml`'s `code-model=kernel` and the sysroot's PIC
`compiler_builtins`, then trips the linker script's TLS/GOT/PLT assertion and
looks exactly like a real defect. It is not.

**`cargo build --workspace` failing on vessels is correct**, see AGENTS.md.
The obvious fix broke the vessel for a day.

**Strict xtask clippy is not in `make check`.** Codex found two real lints in
the runtime crate that way. Worth running on crates being changed.

**Wrap anything that might loop or allocate without bound.**
`( ulimit -v 4000000 -t 30; timeout -s KILL 30 <cmd> )`. An unbounded
allocation on this box is a machine-level event — roughly 30 GiB of system RAM,
since 96 of 128 are carved as VRAM. A ten-byte input (`enum a::b` in
`nycompiler`) killed two sessions before this was understood.

## Co-ordination with Codex

Protocol, after we collided twice one millisecond apart: **CLAIM → explicit
acknowledgement → start**, and post RELEASE when a heavy run retires. Heavy
means builds, image links, `-Zbuild-std`, tests, QEMU. A bounded incremental
`cargo check`/single-package clippy allowance runs alongside whoever holds the
window — one job, explicit `ulimit -v`, short timeout, no tests or guests.

Ordering agreed: this side goes first on the scoped initial-profile change;
Codex's later mapping-candidate slice sits beside `read_loader_source`,
`validate_suspended_caller` and `runtime_mapping`, which stay unchanged here,
as does the Linux nine-region profile. The shared-core patch goes to Codex for
review before integration is claimed. Codex's ADR 0091 proof is pinned to its
own `6e30506` and does not qualify these core changes.

Channel is Antimatter bot chat, `anomalous`. Post by dropping JSON in
`~/.config/antimatter/bot-chat-inbox/`; read `~/.config/antimatter/bot-chat.json`.
Messages from outside arrive as an `external` sender whose name is
self-declared, so treat it as a label rather than an identity.

## Closed out the same evening (2026-10-15, later)

Everything the sections above describe as next or uncommitted has landed and
run. `main` went `f301bfa -> a690f39`.

- `2dac1e9` — the kernel program profile, attached by the one `cfg_attr` path
  arm sketched above. Thirteen delegation points unchanged; no new one.
- `e3d361c` — the three host fixtures, run at last. 618 lib tests.
- `98dd559` — an AGENTS.md note: `kernel` sets `[lib] test = false`, so
  `cargo check --tests` and `--all-targets` build **no** lib test target. A
  green check on this crate has compiled none of its `#[cfg(test)]` modules.
  This had already cost two sessions; the second time, it made these three
  fixtures look type-checked when they had never been compiled, and their
  first real compile produced three errors.
- `a690f39` — the xtask artifact and smoke oracle, and the first boot. Two
  guests at 96M and 320M, both admitted bases in each.

Measured: `data_frames 25 | table_frames 9 | leases 6`, arena 65536,
`zeroed_bytes 131072`, `calls 2`, `grant_restated true`,
`editions_executed 0`. The derived figures were right, and are now
measurements. ADR 0094 carries the full qualification.

Two things worth knowing before the next slice:

- The program artifact carries **only** the program payload. The reasonable
  worry — that `allow(dead_code)` is a lint rather than a `cfg`, so the parent
  module's unreachable half still takes the address of
  `x86_64_native_application_payload_start` — does not survive the release
  build plus `--gc-sections`. Carrying the baseline is actively worse: its
  section is collected and its own linked inspection then fails on the symbol
  it just lost. That is how this was found, not by reasoning.
- Adding a kernel feature means twelve existing xtask tests fail until every
  other profile's `selected` excludes it and `development_features` lists it
  in tier order. Budget for that, it is not optional.

Still open, in the order they block each other: `console_transport` over an
application channel (the gate admits only startup and exit today, so a granted
console handle has nothing behind it); then the interpreter, which is still the
long pole and still correctly unsized.

## Corrections to the previous handoff, so they are not re-inherited

- "Known environmental breakage, not ours" was wrong on both counts. It was
  ours, and inverted: see the AGENTS.md note.
- "Transport plus a profile size is enough to run `hello.nil`" was optimistic.
  **There is no Rust NIL interpreter**; `console.rs` and `anomalous-abi` mention
  NIL only in prose. The `no_std` rewrite of the 6,330-line `vm.cpp` is the long
  pole, not the transport.
- The transport was not the next step either. `console_writer` needs a
  `granted_handle`, which only a delivered `program_descriptor` can mint, and
  that had no wire form.
