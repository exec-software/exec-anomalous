---
title: "Computed Text Log"
date: 2026-11-13
status: qualified-host-profile
---

# Bounded computed text and hexadecimal integers

[ADR 0108](../../docs/adr/0108-add-bounded-computed-text-and-hexadecimal-integers.md)
adds explicit profile 4 from checkpoint `f7f6d7e`, on the isolated
`codex/nil-computed-text` branch. The original checkout and its unrelated work
are preserved. This checkpoint changes host language support, with no native
NIL execution, new kernel privilege, or Linux service advancement.

## Implementation and source evidence

Profile 4 adds exact `STR_CONCAT` with reachable text/text CFG proof and complete
signed-i64 hexadecimal tokens. It inherits scalar temporary locals and bare
integer ADD. Production and raw validation select explicit separate entry points;
older profiles refuse both extensions and profile-4 package tags. Raw witnesses
expose their selected grammar through a read-only property, without gaining any
conversion to production admission.

Reference revision `4dad0512dfd3fa4e351f2816196cb48956f5442e` recognizes optional
ASCII sign plus `0x`/`0X` before floating-point detection. Its signed conversion
requires complete token consumption; negative minimum is valid and positive
`0x8000000000000000` is not. The Rust parser accumulates negatively using checked
arithmetic, preserving that asymmetry without an unsigned bit-pattern cast.
Decimal spelling and errors are unchanged.

The reference loader decodes literals once and the VM concatenates decoded left
and right strings. A separate computed-text value therefore stores a private
span in a fixed 65,536-byte execution-owned arena. Literals retain borrowed
escaped spelling. Every concatenation measures complete decoded lengths before
writing, commits a fresh immutable result, and charges its full result size.
Copying handles through locals and calls preserves aliases; returning from a
helper never releases the bytes. Empty results charge zero, including at full
capacity. No in-run reclamation or garbage collector is introduced.

A shared literal decoder retains all earlier escape behavior, including literal
backslash-plus-b for the supported unknown escape. Computed bytes are never
sent through that decoder. The shared console publication path applies the
same witness identity/authority, whole-payload checks, 4 KiB request bound,
64 KiB acknowledged-output bound and no-retry accounting. Every execution
boundary scrubs the complete arena and existing machine storage. Arena capacity
is distinct from console capacity; `text_limit`, `output_limit`, output policy,
arithmetic faults and service refusal retain earlier acknowledged output.

The new guarded C++ oracle uses the actual pinned VM. Its pre-dispatch checks
restrict STR_CONCAT to text/text and cap cumulative computed bytes at 1 MiB and
live stack/local string bytes at 16 MiB, using a noncopying debug visitor. The
other instruction/frame/output caps remain explicit. An exponential-concat
fixture reaches the exact computed-text guard; this safety check receives no
candidate semantic-coverage credit. Neither a guard refusal nor a timeout can
substitute for a completed finite reference comparison.

The external `reference-source-audit.json` records exact source hashes and
anchors: `numeric_literal.hpp` for prefix recognition, `nil_loader.cpp` for
literal decoding/validation, and `vm.cpp` for concatenation and return lifetime.
No pinned reference source or archive is modified.

## Development evidence and retained correction

Artifacts are outside the repository at
`/home/nathan/Documents/Codex/2026-09-26/l/work/program-text-2026-11-13`.
The first focused flow run passed 64 of 65 tests. Its remaining assertion
incorrectly expected `manifest_unsupported` for profile 1's instruction after
RET. That accepted parser checks its terminal state first and returns
`manifest_syntax`. The test now pins that actual historical outcome; production
behavior was not changed. The failed log and successful retry remain retained.

Focused flow tests, executor tests and host Clippy subsequently pass. The new
proof passes all 55 cases and its unchanged full corpus capture. An independent
checker reads actual process records, full emitted/input bytes and exact tools;
its preliminary expectation for a repeated-output fixture was corrected from
all-x bytes to the authored x/y block repeated 16/17 times. No candidate defect
was found. The independent development audit passes, including original checkout
preservation and matching interpreter inputs to preceding compiler artifacts.

## Boundary proof and corpus ledger

The **55 boundary cases** comprise 34 exact semantic matches, one matching
runtime fault, six finite-reference resource/output-policy contrasts and
14 validation refusals. Forty-one finite reference runs complete. All 55 receive
separate profile-4 production attempts; two intentless fixtures retain production
refusal. Forty-one accepted new-form cases receive additional production and
raw profile-3 refusal checks. Three newly supported complete corpus editions
also retain their earlier-profile refusals.

Directed coverage includes Unicode, empty text, backslash/escape boundaries,
unknown escape preservation, immutable aliases, parameters and returned text,
branches, signed hexadecimal limits, exact arena filling, cumulative allocation
after POP/return, prior output before fault/refusal, per-request output limits
and repeated aliases exhausting total output. Finite policy contrasts are not
semantic matches. Unit tests inspect complete storage clearing and rejected
allocation atomicity.

The untouched upstream runner still executes 97 compiler calls, 38 root cases
and 45 interpreter invocations, with its original schemas, diagnostics, imports
and single telemetry exit-only exception. Raw profile 4 matches **seven of 45
invocations**, comprising four roots and three auxiliary executions:

| Complete corpus case | Evidence |
| --- | --- |
| Three source-extension selections | Existing matching auxiliary invocations, identical 306-byte edition |
| `hello.matrix` | Existing matching root, 298 bytes |
| `scalar-subset-in-ny.ny` | New matching root, 10,881 bytes, 113 console bytes |
| `scalar-subset.matrix` | New matching root, 10,881 bytes, 113 console bytes |
| `short-extension.mat` | New matching root, 612 bytes, helper returns `hello matrix` plus LF |

The remaining 38 raw invocations refuse: 29 unsupported forms and nine edition
limits/formats. Production profile 4 independently refuses all 45: 25 unsupported,
nine edition and 11 intent first refusals. The three new corpus successes now
reach production's missing-main-intent refusal after their earlier helper bodies
become supported. No production policy is waived; no corpus bytes, helpers or
expectations are rewritten. Known positive edition sizes/hashes and exact
coverage/refusal inventories are a regression contract. Corpus comparison
preserves the upstream process-exit-byte and console contract; full-i64 results
are independently checked by the boundary oracle.

## Independent receipts and native continuity

All five final differential proofs have independent retained-evidence receipts.
The new `qualification/independent-text-summary.json` has SHA-256
`82b98fff1403c933fce619325f3939e71ec3c8c7129faf12e87bfaaaacc481b7`. It verifies the exact 55-case inventory,
41 finite references, both profile-4 lanes, 41 earlier-profile boundary pairs,
three earlier-profile corpus pairs and all complete compiler/interpreter records.
The raw candidate SHA-256 is
`f6d194e46ec588409fcd30386e69fc5d303d5a48c46600b53c43ac443c50ce8a`;
the production profile-4 candidate SHA-256 is
`e205a42a1f48c9a119020b3266c06b05caffbd3d0a58ce8f1e6a3b5a24406fd2`.

Fresh native admission TCG evidence passes at 96/320 MiB. It retains four admitted
and six refused packages, 12 edition reads, one fixed loader marker and zero
NIL executions, with complete cleanup. Five admission artifacts and all 11
previously reviewed emitted-code source inputs are unchanged. The independent
native checker rejects all 8,614 transcript/report mutations. Historical AMD
KVM evidence is reused only for those byte-identical artifacts; no fresh KVM
or VirtualBox run is claimed.

## Final qualification

All **32 final qualification jobs passed on their first attempts**, without
guard interruption. The retained inventory includes all
four unchanged earlier proofs, the new text proof and oracle guard, explicit
freestanding host-feature exclusion, all repository-required checks and fresh
native/Linux continuity tests. The final freeze covers **685 non-Markdown files**;
`qualification/source-sha256.json` has SHA-256
`9fea882a547cf199244fda8513bb10f5d05925984e125001ef0d325aab658e76`.
Candidate binaries are copied out of shared Cargo target storage before proofs.
Builds remain serial under the established memory/process-group guard. Existing
user applications and VM sessions are not stopped.

The independent final audit verifies all five proofs (126/77/61/27/55 cases),
exact reference/candidate tools, complete corpus captures, source freeze, oracle
safety refusal, freestanding feature exclusion and native/Linux continuity.
The serial jobs total 22.0 minutes. `qualification/summary.json` has SHA-256
`4835206cab0678e7cc96b988c1bc81671824e8f2be0a4dd51145dc5bb9284203`.
The original tracked diff remains SHA-256
`28d1d1f342846a1cbf8e637b0de1e6eedf71cfccd600e0825b347edce77408c3`,
and seven unrelated original-checkout files remain unchanged.

Host suites pass 648 default and 731 all-feature kernel tests, 425 tooling,
37 native-runtime, 18 manifest, 45 default executor and 51 default flow tests.
The opt-in suites pass 50 executor and 65 flow tests, with one and three
compile-fail doctests respectively. Python harness counts are 6/5/4/5/5.
Formatting, warning-denied host/freestanding Clippy and artifact checks pass.

Fresh native delivery preserves boot-image SHA-256
`00f27bd5071f768c15e863ec872d54644a3c471bff15eb954122e98730bb691b`.
At 96/320 MiB, Linux still records 12 requests and 11 returns, stopping at the
unanswered sequence-12 `mmap(0x100028000, 0x189000, 5, 0x812, 3, 0x28000)`.
No Linux process exit or new application compatibility is claimed.

Full interpreter qualification and native execution remain false. A future native
embedding still needs production-admitted dispatch, measured code/stack/arena
geometry, task-bound effects and complete cleanup, in addition to full corpus
qualification.

## Next language slice

The smallest complete remaining target is `control-flow.matrix` (invocation 17,
capture 0087): 7,821 bytes, 189 instructions and two functions. Its remaining
forms are a text array literal, LEN and GET_INDEX. A bounded typed-array profile
could exercise this unchanged edition, preserving distinct handles, source order,
aliases and text lifetime in fixed per-execution pools. No prospective coverage
is credited here.

The pinned default reference returns element-default text/integer values for
out-of-range array reads; strict bounds is a separate optional mode. Index
semantics must follow the selected reference mode, with signed checks before
host indexing. Extending value kinds also requires restricting console validation
to supported integer/text payloads, so generic stack popping cannot accidentally
admit array printing. Mutation, nested arrays, pointers and freeing should remain
separate work in an initial immutable-array subset.
