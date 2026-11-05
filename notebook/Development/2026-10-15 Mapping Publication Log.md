# Mapping publication work — 2026-10-15

Starting from accepted documentation commit `73f538a` and qualified code
`3e3239c`, on isolated branch `codex/runtime-mapping-publication`. Claude's
ADR 0094 work on main remains separate. The intended checkpoint is ADR 0096:
retain an actual source owner across native returns, publish one bounded caller
subtree, inspect through composed user-copy, then withdraw and release it while
the Linux caller remains suspended. This is not a Linux mapping reply.

Evidence belongs in
an external validation directory (not distributed).
The memory guard and checkout adapter are unchanged: one build/test thread,
4 GiB soft limit, 6 GiB hard limit, 512 MiB swap and the existing host-memory/PSI
checks. Native operations 26–29 are new; operation 25 stays unchanged. No new
guest execution or qualification is established by this implementation note.

The native proof adds thirteen lifetime calls (four successes and nine
rejections) and two unchanged-context queries. Expected normal totals are 852
native gates and fourteen context queries, with ten Linux returns and sequence
11 still pending. These are expected values until measured evidence passes.

The first preflight attempt stopped at formatting because the feature-selected
dispatcher module needed an explicit child path. That failed attempt is retained.
The corrected `preflight-v2` completed all six jobs: formatting, 75 protocol
tests, 385 host tests, 646 kernel tests, and strict host/kernel lint. Peak guarded
memory was 1,136,336,896 bytes; the host OOM counter remained two and no scope
OOM or pressure event occurred. This establishes a tested source checkpoint,
not emitted-code or physical guest qualification.

Source review added exact original-tree verification before candidate release,
refusal of operation 25 while a retained owner exists, and an aggregate teardown
guard before any root unregistration. The 535-page composed-copy host test
crosses the 512-page leaf boundary, corrupts the later translation, rejects a
partial result, and verifies tail refusal and restored accounting. The actual
source moves by value; its readers and access borrows do not cross native gates.

## Accepted qualification — 2026-10-15

The pushed source checkpoint is `924e62c061505902955a6999aaf93123bce7f5b8`.
The fresh freeze covers 568 non-documentation files. Focused artifacts are kernel
`855ae0ec8c3fe2e5570a7b71816dcdf8691c8267df72f3fc252031577a967cca`,
native ELF `ffe8c01ebbf711cc2ac11b6655dbd48523c4bea8b547cfaa4ed40651d25ebec2`
and loader image `2d34ce71025a138fa892cb86d32225ba89fad9e1bea6ae481d8156e56c05c6f6`.
Preflight-v2 passed 75 protocol, 385 xtask and 646 kernel tests plus formatting
and strict lint. Preflight source identity is explicitly root-attested against
the later freeze, not a contemporaneous pre-test inventory. Preflight-v1 remains
the failed formatting/path-resolution attempt; there was no guest retry.

Native owner and independent semantic reviews cover the fresh emitted code;
its user-stack bound is 3,296/8,192 bytes. Kernel closure contains 286 reachable
functions, no unresolved edge and no cycle. Bootstrap bound is 40,208/65,536,
with 25,328 bytes remaining; privileged native bound is 11,104/16,384, leaving
5,280 bytes. Adapter-authored semantics and root crossreview are distinct:
root independently reviewed the adapter but authored the lifetime controller
and integration. This is not a claim of whole-change reviewer independence.

TCG 96 MiB, TCG 320 MiB, KVM 96 MiB and KVM 320 MiB all pass identical reviewed
artifacts across seven lifetimes and 197 strict records per boot. The normal
lifetime records 852 native gates, fourteen context queries, one successful
retain/publish/inspect/release and nine lifetime refusals. The candidate has
535 populated pages in a 545-page reserved extent, five table frames, 2,190,608
source bytes, 752 checked zero bytes and ten absent pages. Source/destination
FNV is 18142536360841512133. Original mappings and caller state are preserved;
withdrawal, lease cleanup, circle accounting, scratch wiping and source recovery
complete. The six earlier file-service fault lifetimes do no mapping work;
this is not new live-owner guest fault injection. The separate legacy allocator
probe still records two borrows and one nested denial, not total callbacks.

`required-check-v1` passed `make check`; `qualification-v1` passed the remaining
29 commands, including affected owned, ELF/Linux and editor/workspace tests and
the final image restoration. All six provisioning refusals pass before a guest
call boundary; they do not expose independent final physical cleanup accounting.
VirtualBox-v1 is retained as failed: `application_cpu_amd_error_pointers`, nine
records, no loader-execution pass, and verified VM poweroff. There are exactly
two failed scopes: preflight-v1 and virtualbox-v1; neither is relabeled as pass.

The final memory audit covers fifteen scopes and sixty attempted jobs, with
nonoverlapping guarded job intervals. Maximum scope peak is 4,297,064,448 bytes;
minimum sampled host availability is 12,450,877,440 bytes. Make check recorded
10,340 soft-limit events; max/OOM/OOM-kill/group-kill counters are zero. The host
OOM count stayed 2→2. No pressure abort occurred. These claims cover retained
samples, completed runner outcomes and unit journals for this task, not all
unrelated host activity. HAG received a separate CPU-only window between the
matrix and VirtualBox; Claude later announced a user-authorized bounded host-test
overlap. Neither is represented as part of this task's guarded job inventory.

| Evidence file | SHA-256 |
| --- | --- |
| `source-freeze-v1.json` | `8d6de012e792695bcdc1a51b1494d2447205a299c6066f3e4a6da8bcddaf90d0` |
| `focused-v1-artifact/hashes.json` | `1f50c3d17b873d652e64c91a73cb2a0749b3284ada4c56aa54f0a9d08417125a` |
| `fresh-native-review-v1.json` | `a196db505af8c685ba47be0045221df108be4fdda958f6f1c3addc4c89f51d4f` |
| `kernel-emitted-review-v1.json` | `9c0524ce4c00eb67f870838edbe928a34168ca33adb5dd7aeda0ebad1fab0624` |
| `kernel-emitted-semantics-v1.json` | `0c0217fe34023e63155fc9a609c89bc9d42ea35cae9f2d5172c1f5a700a9ec2b` |
| `kernel-root-crossreview-v1.json` | `4039364884415357f1594f3a38355d77db2148ddcc1a367a449fd5a807a4441d` |
| `physical-crosscheck-v1-receipt.json` | `938e6a1bfb2f4fb8b166a07a2dee5b7a3e6c759e703d823c094a86f4656f000f` |
| `baseline-change-review-v1.json` | `6b8b2edcf3b4ed3dbbd948bd0da491d3eca5aa30018d1032faa5b345586371ee` |
| `memory-audit-final-v1.json` | `81182377c0f8f93d0640cb7c7d551c6eec58fee4af85a4e294075bfdaa3758a8` |
| `final-audit-v1.json` | `66b1473a5726a7a2e44220d74fd1a836e20fd0d667dd697ba9ae821620fc9db7` |

The original final auditor remains preserved. The v2 auditor requires a current
root-authored source/runtime review for each actually changed baseline image;
it retains all source, emitted-review, physical, full-matrix and memory gates.
The normal image matches exactly. Each workspace variant differs in nine bytes:
one source-location line byte (`circle.rs` 231→244) and eight Threshold
manifest/kernel hash bytes. The editor image differs in fifteen bytes: seven
source-location line bytes and eight hash bytes. Root checked the decoded file
names, columns and old/current statements; independent review confirmed the
bindings. No prior exception or whole-image equality is inherited.

| Image | Current SHA-256 | Historical SHA-256 |
| --- | --- | --- |
| `anomalous.img` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` | `2f341f569154844f7f37e2fe6166e394d9105862b2297abeaef9baca1be3f6e1` |
| `workspace-dev.img` | `99e5f7c2e0920153f7573d2bcfe84098b94699ec5644de6425bae4e5b7622e3c` | `6e855b537c21399a94cdfba82db3a32173a08c0b32f239914639fed47b2d5eff` |
| `owned-workspace-test.img` | `1040656927c8bc1207bda56ee5230529a755ed759b6d82504d25979a926b269e` | `b3974037a5850dac701f70d484c46082ae14f08137bc06cf08f4dfb403cfad86` |
| `owned-workspace-cpu-test.img` | `edd758159fd0b763e4cbac774539deac164f3e5246383f5525c318a23c520014` | `1fe69150dfd2629796c7d3913de8ba24695f25b81255d5931038bca616d5b045` |
| `native-editor-workspace-test.img` | `d3b9ac2c41ec7120ac69cea6ebae81b261a61161d76e54e12c57d29245ad908d` | `e36ceee6a46fb376ac4b22a1c42df166b5985de2dc3ab3c5b21a1cf63b5e5070` |

The exact editor profile has current direct runtime coverage. Shared circle,
scheduler, application-profile and multi-region ownership regressions cover the
relevant shared paths. Workspace-dev and the distinct one-owner/CPU workspace
profiles were rebuilt/inspected but not directly rerun as workspace smokes;
the baseline review preserves that limit. Byte/source-location observations are
not a general proof of semantic equivalence.

The caller remains suspended at sequence 11 after ten Linux returns. The
read-only, non-executable candidate is never selected as a running caller
mapping. This slice adds no post-publication guest fault injection and proves
no Linux mmap return, main `true` entry, dynamic linking, Linux exit or Antimatter
execution. Next qualify hardware-use ownership, safe caller activation and the
exact authenticated userspace Linux mmap reply, then observe the next interpreter
request. Foreign syscall and ELF policy remain in userspace.
The read-only next-step review identifies hardware-accessed/dirty-bit admission,
active composed copy with an owner-selected ten-mapping profile, fresh invocation
authority after sequence 11, and quiescence/invalidation after actual execution
as the next work. Capture the next interpreter boundary rather than predicting
or implementing its request in advance. The source remains in the runtime owner;
later file reads require their own access decision.

Coordination note: bot chat sequence 59 clarified the NIL Framework umbrella
name. Ny and Matrix retain their language names; folder and executable names may
change later. This qualification changes no frozen toolchain paths or names.
Claude's native-program work on main, including reported commit `2dac1e9`, is
outside this isolated source identity. His 25 data frames, nine table frames and
six leases remain source-derived here until separately executed evidence is
reported. The code and this acceptance must be integrated with that work without
mixing qualification identities.
