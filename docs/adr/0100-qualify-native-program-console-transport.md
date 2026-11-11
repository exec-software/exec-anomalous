# ADR 0100: Qualify native program console transport

- Status: Proposed
- Date: 2026-11-11
- Builds on: ADRs 0048, 0092, 0093 and 0094
- Scope: An experimental console service for the bounded ANPG program profile.

## Context

[ADR 0094](0094-map-a-program-arena-against-a-named-profile.md) qualifies a
program-bearing task and its copied `ANPG` startup descriptor. The descriptor
pairs service grants with handles, but the original profile accepts only startup
and exit. Restating a console handle does not prove that an application can use
it or that the kernel rejects an unauthorized request.

The [Runtime Owner Reply](../../notebook/Development/2026-10-15%20NIL%20Runtime%20into%20Anomalous%20%E2%80%94%20Runtime%20Owner%20Reply.md)
selects console output before bounded file reads and time. It selects a bounded
Rust `no_std` NIL interpreter, with the existing execution corpus as its
differential acceptance gate. This does not require embedding the hosted C++
VM. It does require an independently qualified interpreter and the artifact
identity, derived authority and loader enforcement of
[ADR 0093](0093-carry-a-semantic-manifest-in-the-program-image.md).

This increment supplies the first real application service through the ANPG
profile. It admits no edition and makes no interpreter-compatibility claim.

## Decision

### Keep the service experimental and profile-local

Use the existing `int 0xf4` native program gate. Operation 1 requests a console
write; operations 0 and 5 retain startup and exit. Other operations convey no
implicit service authority. This register contract is not a stable public ABI
and does not replace the separate `int 0xf3` supervised-vessel rite contract.

| Register | Console request | Console reply |
| --- | --- | --- |
| RAX | Operation 1 | Status |
| RDI | Raw console handle | Preserved |
| RSI | User text address | Preserved |
| RDX | Byte length | Committed byte length, or zero on rejection |
| RCX | Request sequence | Committed sequence, or zero on rejection |

The experimental `program_status` values preserve the native packet gate's
numeric conventions: `ok = 0`, `denied = 2`, `bad_address = 3`, `invalid = 4`,
and `closed = 5`. They are not the `rite_status` wire vocabulary.

### Bind authority to the retained live task

The controller constructs [console state](../../kernel/src/arch/x86_64/program_console.rs)
from the exact generational task identity, optional raw handle and admitted
arena before publishing execution. A descriptor decoded or rewritten by
userspace cannot replace that binding. Each gate first validates the actual
running task, saved frame and retained execution owner. Matching a numeric
handle alone never authorizes a write.

The console state rejects absent, wrong or retired bindings. Sequence numbers
start at one and must match the next expected value; zero, gaps, replays and
overflow are refused. Close revokes the handle before task retirement and
mapping release. Publication is serialized under the existing sole-BSP,
nonreentrant dispatch interval.

### Copy and validate the whole request before output

Accept one through 4,096 bytes wholly inside the task's admitted arena, with
checked address arithmetic. The retained execution owner preflights the complete
user range and copies it through its privileged physical access path. It does
not dereference an unchecked user pointer or trust a caller-side text check.

A 4,096-byte scratch buffer resides in the profile's BSS-backed owner rather
than consuming the exception stack. Clear it on entry and every returning path,
including partial-copy failure. These are logical clearing guarantees; this
record does not claim volatile erasure after an abort.

Validate the complete copied payload as UTF-8. Admit newline and tab, reject
other C0 controls, DEL and decoded C1 controls, and preserve legitimate Unicode
continuation bytes. The userspace `console_writer` and the service use the
[same text rule](../../crates/anomalous-native-runtime/src/console.rs), while the
service independently checks its trusted copy.

Only then invoke the synchronous serial publisher. Commit the sequence and byte
count after publication completes and return both values to the task. Rejected
requests publish nothing and consume no sequence. The publisher must complete
or never return; a partial or uncertain serial failure is fatal in this proof,
so it cannot return a retryable error after emitting a prefix. A recoverable
output sink will require its own publication transaction.

### Verify application-visible results and teardown

The [native client](../../vessels/anomalous-native-program-client/src/main.rs)
uses the checked writer for two successful messages. Its gate stub returns the
actual status, sequence and length; the client requires an exact reply before
it issues the next write. The first buffer begins at arena offset 4,095, so
actual copying is unaligned and crosses a page boundary.

Raw requests separately exercise wrong and absent handles, stale authority in
the ungranted scenario, zero/skipped/replayed sequences, empty and excessive
lengths, overflow and out-of-arena addresses, malformed UTF-8, ESC and a decoded
C1 control. Rejections are ordinary replies, not expected user faults.

Retain the two separate task lifetimes and arena bases `0x804000` and
`0x818000`. The first grants the console; the second grants none. Each first
observes its complete zeroed 64 KiB arena and restates the ANPG grant. Exit
revokes the console and completes the existing task, mapping, CPU-state,
transition-stack and allocator cleanup before reusing the profile owner.

## Evidence contract

Keep the twelve existing structured event names and their order. Require exactly
these two raw UTF-8 lines after all eight boot/interrupt records and before the
profile's `ready` record:

```text
Native console: ready — ✓
Native console: reply checked
```

Each line contains 30 UTF-8 bytes including its LF, for 60 committed bytes.
The [strict oracle](../../tools/xtask/src/native_program_smoke.rs) accepts LF or
serial CRLF framing and rejects missing, duplicate, moved, changed or additional
text. It validates exact fields and types rather than inferring success from
the mere presence of a greeting.

| Evidence | Granted scenario | Ungranted scenario |
| --- | ---: | ---: |
| Total gate calls, including startup and exit | 18 | 4 |
| Console attempts | 16 | 2 |
| Committed writes | 2 | 0 |
| Rejected requests | 14 | 2 |
| Committed bytes | 60 | 0 |
| Last committed sequence | 2 | 0 |

Each scenario must exit once, restate the exact grant, and have zero invalid
gate operations. The final record requires two writes, sixteen rejections,
60 bytes, verified console output, 131,072 observed zeroed arena bytes, complete
resource restoration and revoked authority. It states `editions_executed = 0`;
readiness states `edition_mapped = false` and `console_transport = true`.

Host tests must cover authority and sequence rejection, complete-copy/text
admission, publication ordering, scratch clearing, exact replies and hostile
transcript mutations. Native validation must exercise both arena bases at both
96 MiB and 320 MiB, then the required and affected regressions. The bounded
implementation passed all 27 final jobs on 503 unchanged source files, including
both RAM sizes under TCG/Nehalem and AMD KVM/host with the same image. The
[Native Program Console Log](../../notebook/Development/2026-11-11%20Native%20Program%20Console%20Log.md)
records exact transcripts, host counts, image identity and failed attempts.
Measured qualification does not change this record's Proposed status.

## Remaining work

Edition delivery through a bounded file service, semantic-manifest derivation
and enforcement, and differential qualification of a bounded Rust interpreter
remain open. The pre-existing handwritten `edition_interpreter.rs` working file
is preserved and unlinked. Its permissive parsing and absent manifest/corpus
qualification cannot establish NIL application readiness.

The Linux ELF adapter and Linux personality remain separate from this native
service. Fresh loader-input qualification observes sequence 12 as
`mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)` at both RAM sizes, after
eleven Linux returns. The interpreter is suspended before this fixed private RX
request receives a reply. [ADR 0098](0098-model-source-backed-executable-replacement.md)
models the historical transition only; its old length was `0x198000`. Live
retained permission replacement for the current request, hardware execution and
protection tests, and authenticated reply remain the next gate. Dynamic linking,
main executable entry and a Linux exit are later evidence.

This record adds no general console device, terminal input, asynchronous output,
multi-client service, stable syscall ABI, arbitrary artifact launcher, or
privileged Ny migration.
