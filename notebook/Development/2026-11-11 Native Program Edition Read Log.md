# Native program edition reads

Date: 2026-11-11

Status: Qualified and accepted for the bounded TCG/KVM edition-delivery profile.
All thirty final jobs pass; edition interpretation remains outside this proof.

## Scope and baseline

[ADR 0101](../../docs/adr/0101-deliver-bounded-native-program-edition-bytes.md)
adds bounded reads from one immutable source to the ANPG program profile.
The increment begins at `c3a774d`, following the qualified
[console transport](2026-11-11%20Native%20Program%20Console%20Log.md).
That previous increment's passing results do not qualify this source-delivery
change.

The native client remains a Rust fixture with a 256-byte ANPG descriptor and
a 64 KiB arena. Preliminary linker sizing found a 5,605-byte client payload,
requiring an explicit expansion of the named program profile from one to two
RX pages, padded to 8,192 bytes. Final retained artifacts confirm that size.
Two-page admission requires the retained program kind and exact
single-task geometry; the generic one/eight-page gate and native application's
eight RX pages with a 32 KiB arena remain unchanged.

The delivered source contains synthetic binary data. No NIL edition is
interpreted, no semantic manifest is admitted and no Linux execution milestone
is added by this work.

## Implemented boundary

Operation 2 on `int 0xf4` receives the edition handle in RDI, arena destination
in RSI, capacity in RDX, exact sequence in RCX and source offset in R8. It returns
status in RAX and committed sequence/length in RCX/RDX. Capacity must be from
one through 4,096 bytes. The existing ANPG wire encoding is unchanged; its
`file_read` bit gates the typed `edition()` accessor and the controller's live
binding decides actual authority.

The kernel validates the complete capacity against the arena and preflights all
of its writable pages before source access, even if a short read or EOF will
deliver fewer bytes. Checked source-offset plus capacity overflow is invalid.
A valid offset at or beyond EOF succeeds with zero bytes and consumes its
sequence. Reads use explicit offsets and no mutable file position.

Nonempty reads stage an exact provider result in supervisor BSS scratch before
publication. Provider failure or a mismatched length returns `unavailable = 6`.
Other statuses retain `ok = 0`, `denied = 2`, `bad_address = 3`, `invalid = 4`
and `closed = 5`. A publication error guarantees no destination change; a
privileged hardware fault remains fatal. Nonempty reads commit sequence and
byte accounting only after publication; EOF commits after preflight without
source access or copyout. Scratch is cleared before source access and on every
returning path. Bytes after a delivered prefix remain unchanged.

The kernel reuses the sealed-source owner and generic physical-copy mechanisms.
It does not import the portable userspace file service's path namespace,
descriptor table or foreign-file policy. This launch exposes exactly one
controller-bound source through its edition handle.

## Provisioning and retained ownership

The host generates an 8,195-byte file, byte `i` equal to
`(i * 37 + 11) mod 256`, and supplies it separately from the native client image
through fw_cfg item `opt/anomalous-program-edition`. The trusted length and
FNV-1a-64 value `0x84ca7df1efd6b401` are independently fixed in the controller
and proof oracle. FNV is a corruption/evidence check, not authentication.

The existing configured provisioning controller validates the input and retains
the sealed snapshot. Import completes and removes its alias before execution
preparation installs the shared physical-alias slot. The successful source owns
three data frames and one bound lease, separate from the task's 26 data frames,
nine table frames and six leases. The 26 task frames comprise two code, two data,
sixteen arena, two user-stack and four transition frames. Granted totals are
29 data frames, nine table frames and seven leases; the ungranted scenario
imports no source and retains only the task geometry.

Exit closes the service binding before task retirement. Execution and its alias
are finished before source cleanup installs its independent alias. Cleanup
releases the source only after clearing and readback; partial ownership remains
retained on failure. The final proof recovers the pre-launch allocator, task,
circle, scheduler and CPU state.

## Allocator and guest-transfer review

Review found that the inherited ANPG profile entered userspace while retaining
its bootstrap allocator callback and a mutable program reference. This increment
also separates preparation, execution and teardown according to
[ADR 0091](../../docs/adr/0091-release-allocator-borrows-before-userspace.md).
Preparation must return from the allocator callback before task selection,
dispatch publication and guest entry. Only raw pointers to stable retained
storage and copied observations may cross the nonlocal transfer.

After terminal dispatch restores and withdraws execution, validate the bootstrap
RSP, root, privilege stack, interrupt and CPU state before acquiring the allocator
again for cleanup. The first authenticated edition request with a preflighted
destination must independently demonstrate two sequential ordinary allocator
borrows, with one nested borrow refused as `allocator_busy` without entering its
callback. Live accounting and the complete saved frame/request remain unchanged.
The ungranted lifetime records no such borrows. All four successful final boots
meet these checks; no allocator pointer escapes a synchronous callback.

## Verified native evidence

Every successful TCG and KVM boot records the following exact observations:

| Evidence | Granted, `0x804000` | Ungranted, `0x818000` |
| --- | ---: | ---: |
| Total gate calls | 39 | 7 |
| Console attempts / writes / rejections | 16 / 2 / 14 | 2 / 0 / 2 |
| Console bytes | 60 | 0 |
| Edition attempts / reads / rejections | 21 / 7 / 14 | 3 / 0 / 3 |
| Delivered edition bytes | 8,203 | 0 |
| Nonempty source reads | 5 | 0 |
| Ordinary allocator service borrows | 2 | 0 |
| Denied nested allocator borrows | 1 | 0 |

Two committed reads are EOF receipts. The byte total includes rereads; it does
not imply an 8,203-byte source. The client independently verifies returned
bytes, exact receipts, untouched tails and rejection canaries. Cross-page
destinations exercise actual copyout rather than only a host range model.
The two console messages retain their exact 60-byte output.
Final evidence includes `allocator_available = true` and exact resource
restoration after both lifetimes. Every edition request compares the complete
176-byte saved caller frame with its authenticated entry snapshot before writing
reply fields, including rejected requests. Every final success record reports
`edition_frame_preserved = true`. Earlier sizing or
preliminary boots without that check cannot qualify the final implementation.

The final evidence keeps `edition_mapped = false` and
`editions_executed = 0`. The delivered data enters the writable arena through
copyout; the retained source is not mapped into userspace or executed.

## Verified refusal evidence

The smoke target retains separate 96 MiB boots for missing input, a source one
byte short and a same-length source with byte 4,096 changed. Recorded reasons
are `program_source_missing`, `program_source_size` and `program_source_hash`.
Each case releases any partial source and restores allocator accounting
before reporting refusal and taking the expected failure exit.

The refusal oracle requires eight boot records, then
`kernel.native_program_test.source_rejected` at info level with
`source_released = true` and `resources_restored = true`, followed by the matching
`kernel.native_application_test.failed` fatal record. Expected QEMU process
status is 35. Both backends pass all three refusal oracles. These deliberate
failure exits are retained and attributed separately from successful delivery;
the artifact verifier permits only their exact reason and cleanup pairing.

## Final qualification

The artifact verifier passed at `2026-11-12T00:19:30Z` (November 11 local time).
All thirty jobs pass on their first final-matrix attempt: three focused suites,
one native image build, twenty-five standard targets and the additional KVM
native-program target. All 598 tracked and untracked non-Markdown source files
match the frozen manifest, including the complete selected file set. Builds and
VMs ran serially with one Cargo build job, one test thread and the host-memory
guard; every final result reports no guard failure.

| Suite | Passing tests |
| --- | ---: |
| Kernel, default features | 644 |
| Kernel, all features | 727 |
| xtask | 402 |
| Native runtime, full and focused | 35 |
| Focused program state and geometry | 24 |
| Focused native program tooling | 12 |

All four successful native boots meet the exact receipt table above. Each has
twelve structured records and two exact UTF-8 console lines, placed between
the eight boot records and native `ready`. Each backend also has three separate
96 MiB refusal boots with ten structured records apiece, for 54 structured
records per native target and ten native-profile boots in total.

| Backend / CPU | RAM | Guest vendor | Restored free frames |
| --- | ---: | --- | ---: |
| TCG / Nehalem | 96 MiB | GenuineIntel | 24,167 |
| TCG / Nehalem | 320 MiB | GenuineIntel | 81,511 |
| KVM / host | 96 MiB | AuthenticAMD | 24,167 |
| KVM / host | 320 MiB | AuthenticAMD | 81,511 |

The verifier derives free-frame consistency from boot accounting and checks the
57,344-frame RAM-size delta; it does not hard-code these observed free counts.
Timer observations are bounded, not performance claims. The recorded per-scenario
tick pairs are `(2, 1)` for both TCG sizes, `(19, 13)` for KVM at 96 MiB and
`(19, 14)` for KVM at 320 MiB.

Frozen xtask source establishes the TCG/Nehalem command selection; TCG command
arrays were not separately recorded. All five KVM command arrays are retained:
missing, short and corrupt sources at 96 MiB, then successful full-source runs
at 96 MiB and 320 MiB. Each uses `-accel kvm -cpu host -machine q35`, the same
image and the exact source path where applicable. Qualification is limited to
these QEMU TCG/KVM profiles; no VirtualBox run was made for this slice.

### Retained attempts and emitted review

The first preliminary native smoke attempt failed at link time with
`native program RX exceeds its single code page`; its command exit was 2.
An artifact-only measurement link established the 5,605-byte client size. The
production linker, packaging oracle and retained program geometry were then
changed together to two RX pages. Admission tests keep that expansion restricted
to the named single-task program profile. No generic two-page launch or change
to native-application geometry was admitted.

The second preliminary native smoke passed after that change but before the
complete saved-frame check was added. Both attempts remain in the parent bundle,
with their logs and result records; neither is reused as final qualification.
The final thirty-job matrix has no failed attempts.

Read-only review of retained emitted artifacts found no blocking defect in the
inspected transport, source ownership, copy/reply ordering or CPU handling.
The retained prebuilt, TCG-tested and KVM-tested 16 MiB images are byte-identical.
The full 143,360-byte reviewed kernel occurs exactly once at image offset
`0xe00`, ending at `0x23e00`. The 5,605-byte client RX artifact matches both its
ELF segment and the kernel payload at `0x120980`; the verifier independently
checks its startup, R8 request transport, RAX/RCX/RDX reply stores and UD2 marker.

The emitted review confirms allocator release before guest entry, complete
176-byte frame comparison before edition reply writes, BSS scratch clearing,
and execution retirement before source cleanup. It measures relevant prologues
and finds no new page-sized gate local. These observations are not an exhaustive
maximum-stack, reference-aliasing or call-graph proof. The reviewed kernel retains
the existing scalar/FXSAVE CPU profile; this slice does not qualify AVX or an
extended XSTATE profile.

### Reproduction and artifact identities

The retained bundle is `program-edition-2026-11-11/qualification`; generated
artifacts remain outside version control. `jobs.json` and the literal job list
in `run.py` specify the same thirty commands, and `results.json` records their
complete ordered results. The focused commands and image build are:

```sh
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
cargo test -p anomalous-native-runtime
cargo test -p kernel --lib --features native_program_test program
cargo test -p xtask native_program
make native-program-image
```

The standard targets run in this order:

```sh
for target in \
    check smoke fault-smoke circle-smoke control-smoke rite-smoke \
    scheduler-smoke supervision-smoke shell-vessel-smoke memory-smoke \
    interrupt-smoke input-wait-smoke shell-smoke \
    semantic-surface-transport-smoke semantic-presentation-smoke normal-exclusion \
    native-program-smoke native-runtime-smoke native-application-smoke \
    elf-application-smoke linux-application-smoke linux-loader-entry-smoke \
    owned-source-smoke workspace-smoke ny-vessel-smoke
do
    make "$target" || exit
done
```

For the thirtieth job, set `edition_evidence` to the retained qualification
directory and run
`PATH="$edition_evidence/kvm-bin:$PATH" make native-program-smoke`.
The wrapper changes only accelerator/CPU selection and records the five QEMU
command arrays. The three supplied-source variants use
`-fw_cfg name=opt/anomalous-program-edition,file=<fixture>`; the missing-source
probe omits that option. The normal fixture has 8,195 bytes, the short fixture
8,194 bytes, and the corrupt fixture changes byte 4,096 with XOR 1.

| Artifact | SHA-256 |
| --- | --- |
| `summary.json` | `84c331e310286e9da430bbb50670b294835ee39e69c08d9f6c92b50a3a71ceb6` |
| `source-sha256.json` | `05196c38ef9ba0409818a829ea3a21a90242685cb0b41df541727d77a0659fe1` |
| `verify.py` | `b837389b48b373d518f5761469acd974ae9de488782a44daa4761913781ed341` |
| `native-program-image.img`, `native-program-smoke.img`, `native-program-smoke-kvm.img` | `e611655bf95c6fb4da0b402dd83dc875c3069788d2c299a80ba55d39bd54cb2a` |
| `native-program-kernel.bin` | `2b30462f7bfabf2503364dd21c093ea04cd334a4de39594225fb36d3ba48d81a` |
| `client.elf` | `f6f16c98dd0cd726c166f0833ff384e742026ff1a5a5b7387447e65e8fac0517` |
| `client_rx.bin` | `07199aac2da41383994a61b3559d2e06ae4976457d97c5eb4a399c31327e1536` |
| `emitted-review.txt` | `b2a27e36ee663af81b6abcbf0f0caa90a63eaec5107bea536bf5136b895bb466` |
| `native-program-smoke.log` | `73a896e62bf8f7019e91b2032b512df24db13e6fb25b09a3e0fbe7263d30cc15` |
| `native-program-smoke-kvm.log` | `af37d87269508e5883ecb62f1718e000ddc4dae18bbbd51da80c3f15c0d80b78` |
| `check.log` | `2c99b3d88735229b2b7697af798c9a839fb786c71b09dd67eb26aa295e08e03f` |
| `kvm-invocations.jsonl` | `cc7dbc6f6499ace47e7c0470683ff204d3443d179201697d148e43666870d182` |

The verifier checks the source manifest and its exact file set, all final command
statuses and order, complete native success/refusal transcripts, artifact hashes,
image equality and reviewed kernel/client binding before atomically writing
`summary.json`. It retains preliminary attempts separately from final success.
ADR 0101 accepts this bounded delivery checkpoint without declaring a stable
public ABI or interpreted-application compatibility.

## Remaining native gates

ADR 0093 still requires a reproducible semantic-manifest format and loader
admission of artifact-derived authority, intent and identity. The selected
bounded Rust interpreter must then pass the unmodified `matrix.execution`
corpus before interpreter compatibility is claimed. General file access and
time remain separate service work. This delivery proof establishes the bounded
source path on which those later steps can build.
