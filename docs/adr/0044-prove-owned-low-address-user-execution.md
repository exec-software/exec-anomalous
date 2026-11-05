# ADR 0044: Prove owned low-address user execution

- Status: Proposed
- Date: 2026-09-27

## Context

[ADR 0043](0043-prove-resident-mapping-activation.md) makes two owned low-address
user layouts active while retaining explicit supervisor residency and privileged
physical access. Its six protection faults originate at CPL0. It does not enter
a user task, validate a resumed user stack, or carry live task identity through
terminal fault and exit.

The next bounded proof must connect that retained mapping ownership to actual
CPL3 execution and the existing circle/task lifecycle. It must admit executable
bytes and user stacks independently of the earlier fixed-address scheduler
fixture. The experiment remains part of the removable `memory_test` feature;
it does not define the native artifact format, a foreign ABI, or a general
process-launch service.

## Proposed decision

### Admit the entry and stack without granting execution authority

Add a pure checked
[`user_entry_plan`](../../kernel/src/arch/x86_64/user_entry_plan.rs) over the
existing user mapping plan. Entry and resumed instruction addresses must belong
to the declared initialized executable-byte extent. Page membership alone does
not admit uninitialized padding or executable gaps. Require a writable,
nonexecutable user stack with explicit absent guards, a 16-byte-aligned initial
stack pointer with 256 bytes of headroom, and checked 8-byte-aligned resumed
stack pointers within the admitted stack-pointer bounds.

These checks establish geometry and permissions. They do not create a task,
select a root, establish the provenance of copied instructions, or grant rights.
The feature-owned fixture is copied into retained data frames before activation;
its exact entry, report, terminal, and continuation sites are separately checked.

### Retain two roots and real execution identities

Construct two distinct mixed roots using the existing resident profile and
physical-access owner. Each root contains three user regions:

- one RX code page at `0x0040_0000`;
- two RW+NX private data pages at `0x0080_0000`; and
- two RW+NX stack pages near the lower canonical user limit, with absent guards.

Each root owns five data frames and nine table frames, including its supervisor
leaf table. The totals are ten data frames, eighteen table frames, and eight
active/bound leases. The explicit supervisor section/stack mappings and the
borrowed high physical alias retain their preceding contracts and limits.

Use real `circle_table` and `task_table` identities, with three mapping children
attached to each circle. Start the two tasks without adding them to the timer
scheduler. The retained root session borrows the mapping owners and physical
access through the complete transition; mapping cleanup cannot run while those
borrows are live. Task, circle, attached-context, and generation accounting must
be restored after terminal handling and mapping withdrawal. A numeric fixture
label is not a substitute for those lifecycle identities.

### Enter and resume a bounded GPR-only fixture

The private native proof lives in
[`user_mapping_test.rs`](../../kernel/src/arch/x86_64/user_mapping_test.rs), with
its narrow entry/return assembly in
[`user_mapping_entry.S`](../../kernel/src/arch/x86_64/user_mapping_entry.S).
Preserve the Rust continuation, install the admitted user entry/stack, clear
exposed general-purpose registers, and keep IF clear. Set CR0.TS for the GPR-only
fixture and restore its prior state before returning to ordinary Rust execution.
The existing separate resident 16-KiB privilege stack supplies TSS.RSP0; validate
its residency, preserve and check that selection, and wipe the used privilege
stack after each terminal user return. Keep the preceding BSP/NMI and unsupported-CPU-control
restrictions.

Select `bootstrap -> B -> A -> bootstrap`, making three root changes. Enter B
first. Two exact feature-local UD2 reports resume B at different noninitial user
stack pointers; the first occurs inside a nested call. Its subsequent read of supervisor kernel text at
`0x0010_0000` must produce the exact present/user/read page-fault error 5 and
terminate only B. Enter A afterward; A produces two resumed reports and a
separate exact UD2 exit. Check distinct private values at the same user virtual
addresses and continued A progress after B's contained fault.

The feature handshake is private test machinery, not a syscall ABI. Each report
or terminal event must match its live task/circle, selected root, proof phase,
vector/error, instruction site, selectors, flags, and admitted complete saved
stack state. Instruction advancement is allowed only for the specific known
report instruction and admitted continuation. The supervisor-read fault must
match its exact CR2 and faulting site. Any unexpected exception keeps the fatal
path and retained ownership rather than accepting a partial match.

The shared
[`exception snapshot`](../../kernel/src/arch/x86_64/exceptions.rs) now
normalizes the actual long-mode hardware RSP/SS saved words for CPL0, CPL3, and
IST entry. The complete frame is 176 bytes including the fifteen saved GPRs,
vector/error, and hardware frame. Returning user validation must use the saved
user RSP, not the kernel exception-frame address or the initial entry RSP.
Fixtures must exercise ordinary CPL0, CPL3, and IST snapshots so this change is
also valid for existing diagnostic and interrupt consumers. This shared
correction can change normal, dispatcher, and workspace image hashes. Image
comparisons must report measured results; feature exclusion is verified directly
rather than inferred from unchanged bytes.

### Complete terminal ownership before reuse

Record B as faulted and A as exited through their real task lifecycles. Before
reclaiming mappings, restore bootstrap, withdraw registered roots, and end the
activation borrows. Validate the owned trees with only the previously admitted
hardware A/D additions. Complete mapping cleanup and mapping/address-space
child detachment before destroying their task/circle identities; prove stale
generations are retired and no attached contexts remain. Clear saved
continuations, restore TS, verify the preserved TSS selection, remove the
temporary physical window, and return all allocator accounting to its initial
values.

Failure remains attributable and bounded. An unexpected native error reports a
static failure reason and halts while the retained owners remain live. This
experiment does not add fallible automatic destruction, asynchronous teardown,
a general user-copy recovery path, or arbitrary task exception policy.

## Validation and evidence contract

Preserve every preceding memory-proof record. Add exactly three records after
`kernel.memory_test.resident.released` and before the existing `passed` record:

| Event suffix under `kernel.memory_test` | Required evidence |
| --- | --- |
| `user.ready` | Two roots, tasks, and circles; three regions per root; 10 data and 18 table frames; eight active/bound leases; canonical code base `0x0000000000400000`; admitted entry, guarded stack, resident privilege stack, and GPR-only profile. |
| `user.verified` | Two entries, four resumed reports, three root changes, one faulted task, and one exited task; denied supervisor read, peer progress, private data isolation, noninitial stack return, restored bootstrap/TS/TSS, and cleared continuations. |
| `user.released` | Initial/free counts equal boot evidence; zero owned frames, leases, tasks, circles, and attached contexts; retired generations, removed physical window, and restored kernel root. |

The native transcript contains twenty-three structured records. The
[`xtask` verifier](../../tools/xtask/src/main.rs) requires twenty selected
lifecycle records in exact order while retaining the existing treatment of
unrelated boot/text records. Every selected record requires exactly its declared
keys, scalar types, and values. The mutation groups cover every new field's
value/type changes, removal, and duplication; missing, duplicate, reordered, or
unknown events; extra private fields; and normal-image exclusion. The legacy
`passed` counts retain their earlier meaning. QEMU success status remains an
independent requirement.

Host validation must cover entry extent and stack admission, near-miss report
and terminal predicates, actual exception-frame normalization, resource lifetime,
and strict transcript rejection. Native validation must observe both task
outcomes, all four resumed reports, supervisor denial, noninitial stack returns,
private data isolation, and complete resource/state restoration at both RAM
sizes under TCG and KVM. Run the complete regression matrix and normal-image
exclusion checks.

Focused validation passed 107 x86 tests, eight memory-transcript tests, and the
freestanding Clippy check. The full 24-target matrix passed in 1,346.9 seconds
of combined target time, including normal-image exclusion and memory,
dispatcher, and workspace proofs under both TCG and KVM. Full host suites passed
398 default kernel, 441 all-feature kernel, and 92 xtask tests. Finalization
verified that all 240 frozen source/configuration files remained unchanged.

Both native backends passed at 96 MiB and 320 MiB with 23 structured records per
run and exact restoration of 24,150 and 81,494 free frames. Each new user phase
observed four resumed reports, one contained supervisor-read fault, one exit,
and reclamation of all 28 owned frames and eight leases. Initial, TCG, and KVM
memory proof images match SHA-256
`71e81fa8385118c56fde3d539b4f7e13179dd19afcf67040a4585068f266a641`.
Normal, dispatcher, and workspace images changed with the shared exception
snapshot correction; direct feature exclusion and all runtime regressions
passed. Neither their identity with older images nor a new VirtualBox run is
claimed. ADR 0044 remains Proposed.

The [Owned User Execution Log](../../notebook/Development/2026-09-27%20Owned%20User%20Execution%20Log.md),
local validation report (`user-mapping-validation.md`; not distributed),
and machine-readable summary (`summary.json`; not distributed)
retain final results, per-target timings/logs, native predicates, and measured
image/source hashes. The latter artifacts live outside the repository.

## Consequences and remaining limits

This is a bounded GPR-only ring-3 execution proof over retained low-address
mappings. It does not supply a serialized native launch contract, ELF/Mach-O
parsing, Linux/Darwin behavior, TLS, extended CPU-state ownership, asynchronous
context switching for these roots, arbitrary user-copy, or permanent kernel
placement. The high physical alias can still write executable physical storage;
the result is not global W^X or production isolation.

The existing scheduler and desktop keep their own profiles. Passing this proof
does not migrate them to the new mapping owner or eliminate the legacy mapper's
rollback-retention gap. General runtime admission must combine this geometry
with the remaining [execution gates](../architecture/execution-readiness.md),
explicit authority, and a separate native execution contract before compatibility
adapters can consume it. Identity/authentication and biometric policy remain
outside this kernel increment.

The next bounded slice should retain preparation, explicit commit, and retryable
cancellation for this same three-region GPR-only launch profile, including
cancellation of an unstarted ready task. Per-step failure injection must show
no runnable task escapes and all resources return before a successful fixture
entry/exit. Keep that input inert and format-neutral, with authority separate;
serialized launch fields and ELF remain later work.

## Alternatives

- **Reuse only the fixed-address scheduler fixture:** insufficient to prove
  owned low-address mappings, initialized executable membership, or resumed
  noninitial user stacks.
- **Treat a CPL0 root switch as user execution:** insufficient because it never
  crosses the privilege boundary or exercises the user fault/exit lifetime.
- **Resume any UD2 or user page fault:** rejected because a private proof must
  require its exact phase, task identity, root, instruction, and complete state.
- **Introduce ELF loading or general scheduling in the same slice:** deferred
  until the mapping/entry/lifetime boundary is independently evidenced.
