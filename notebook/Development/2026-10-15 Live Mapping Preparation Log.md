# Live mapping preparation — 2026-10-15

Continue from the accepted allocator-lifetime checkpoint
`6c242450524a31744364b64a572f4901b22f0097` in the isolated checkout
an external validation directory (not distributed),
on branch `codex/runtime-mapping-prepare`. Claude's primary checkout and pending
ADR 0094 changes remain separate. Antimatter Bot chat coordinates heavy test
windows; no offered slot may silently overlap another agent's claimed work.

[ADR 0095](../../docs/adr/0095-prepare-and-cancel-authenticated-source-mappings.md)
qualifies a feature-private native operation that prepares, inspects and cancels
a detached source-backed mapping while its exact authenticated caller remains
suspended. The veil translates Linux policy into native geometry. The existing
retained source and execution alias remain authoritative; copied grant fields
are not physical-access capabilities. The caller's first libc `mmap` remains
unanswered after ten successful Linux returns.

Implementation and bounded qualification are complete. Evidence is retained in
an external validation directory (not distributed).
The runner and checkout adapter retain their exact prior hashes and memory
policy: 4 GiB soft limit, 6 GiB hard limit, 512 MiB swap, one Cargo job, one test
thread, a 4 GiB host-available floor and the unchanged host/ancestor PSI guard.
All attempted scopes, including failures, must remain in the final audit.

The starting five baseline images are retained qualified build outputs, copied
in bounded chunks and verified against the previous final audit. This is explicit
historical provenance, not a new build. Libc and the fixture's 16-byte startup
seed are independently pinned. The changed userspace veil requires fresh native
emitted review; the preceding loaded-image comparison cannot qualify it.

Source review selected a lexical local mapping owner with a real reader borrow
and two retained 4 KiB scratch buffers. No borrowed owner crosses a userspace
transition, and no failed preparation may return before explicit cleanup.
Constructor temporaries and the expanded synchronous privileged path require
fresh emitted stack analysis before any guest test. Source-level review alone
does not establish those bounds or actual physical allocator behavior.

## Implementation checkpoint

The native veil now probes eight rejected requests, one successful private
prepare/cancel and one rejected replay, followed by a preserved-context check.
The kernel binds the source and suspended caller before allocation, populates
and reads back the complete source prefix, independently walks the candidate
tables, and releases its lexical owner before replying. The transcript oracle
requires a separate mapping-cancellation record for every lifetime.

Guarded `preflight-v1` passed formatting, 72 protocol/policy tests, 382 host-tool
tests, 630 kernel tests and strict host/cross-target kernel lint. Its scope peaked
at 911,355,904 bytes with no pressure-limit or OOM events. Source review found an
overbroad A/D-bit mask in the caller-table diagnostic hash. The hash now covers
exact entries of the inactive owned table arena. After that correction,
`preflight-v2` passed formatting, all 630 kernel tests and strict cross-target
lint, peaking at 412,831,744 bytes without limit/OOM events. The host's existing
OOM-kill counter remained two throughout both scopes.

At that implementation checkpoint, guest qualification was still outstanding.
Fresh binary, stack, BSS and independent emitted reviews, physical tests and the
full regression matrix were required before accepting ADR 0095.

## Accepted preparation/cancellation checkpoint

ADR 0095 is accepted for code `3e3239cd629d9aafd007a6d077e31c843993387b`, bound
to the complete 563-file non-Markdown source inventory. Any later documentation
commit is recorded separately and does not qualify later core changes. Claude's
ADR 0094 work is outside this frozen code checkpoint.

Fresh native inventory binds all 4,222 instructions in 60 functions to the
current ELF. The owner and independent reviewer authored 40 and 33 findings,
respectively, covering the existing full flow and the new operation 25. The
native stack bound is 3,280 of 8,192 bytes, leaving 4,912 bytes. Current kernel
review bounds bootstrap use at 36,896 of 65,536 bytes and the largest privileged
path at 11,024 of 16,384 bytes, leaving 5,360 bytes on that path. The kernel
semantic reviewer authored the population helper; root authored the controller
and owning adapter. Root independently reviewed the helper, while its author
independently reviewed the controller and adapter. Their bound reviews cover
constructor copies, retained scratch storage, source readback, exact owned
caller-table preservation, failed preparation, retry retention and cancellation
before reply. Static review receipts themselves make no physical success claim.

Four passing TCG/KVM boots at 96/320 MiB each retain 190 records and seven
file-service lifetimes. The normal request is syscall 9, sequence 11, with
arguments `[0, 2231696, 1, 2050, 3, 0]`; it remains unanswered after ten Linux
returns. One private preparation and nine refusals include the rejected replay.
The candidate begins at 4 GiB and has 535 populated pages in a 545-page virtual
extent, five table frames, 752 zero-padding bytes and ten absent tail pages.
The complete 2,190,608-byte source prefix and destination both have FNV64
`18142536360841512133`. Readback and independent table walks cover the full
extent and neighbors. The mapping remains unpublished, cancellation completes
without retry, allocator accounting is restored, scratch is cleared and the
caller remains unchanged. Caller-tree hashes are guest diagnostic witnesses;
the emitted review establishes the exact owned-table check. The physical
crosscheck does not require address-dependent tree hashes to match across runs.

The six fault lifetimes report zero mapping work and retain their prior fault
windows and peer survival. The normal lifetime records 837 native calls and 12
context queries; the new ten operation-25 calls and one context query are
separate from the earlier two allocator borrows and one nested denial. No
Linux mapping address, main `true` execution, Linux exit or Antimatter execution
is claimed.

`qualification-v2` passes all thirty required/affected make targets with the
configured libc and startup seed pinned on every job. The complete matrix runs
from 22:26:10 to 23:01:12 UTC on 2026-10-15; `make check` finishes in 1,068.26
seconds. Its commands and environments match the original matrix. Only the
outer `make check` deadline changes from 1,800 to 3,600 seconds; the other 29
outer deadlines remain 900 seconds. Test assertions, source pins, the memory
guard and inner guest deadlines are unchanged. All six provisioning refusal
cases pass. Their receipts explicitly do not independently establish final
physical-resource cleanup accounting.

## Retained failures and memory observations

`kvm-v1` remains failed: its 96 MiB guest exceeds the unchanged 180-second
per-guest deadline after four of seven lifetimes. Its normal mapping case had
completed, but that partial transcript is not a successful backend qualification.
The separate `kvm-v2` retry qualifies both memory sizes with the same guest
deadline, wrapper and image pins. No causal claim is made about the first delay.

`qualification-v1` remains failed: its outer 1,800-second deadline terminates
`make check` while the cross-target kernel clippy command is active. The retained
result is exit -15 with `aborted.type=deadline`; it is not a compiler diagnostic
or a memory-pressure guard abort. The complete replacement matrix passes as
`qualification-v2`; the failed attempt is not relabeled or deleted.

`virtualbox-v1` remains a separate failed semantic run. Its retained fatal record
reports `application_cpu_amd_error_pointers`, and the adapter verifies the VM is
powered off. The receipt has `semantic_qualified=false`; VirtualBox has no
loader-execution pass for this checkpoint.

The successful regression scope peaks at 4,296,847,360 bytes, with minimum host
available memory of 15,049,986,048 bytes. It records 83 soft-limit events, zero
hard-limit/OOM events, no memory-pressure abort, and a complete stopped monitor
without runner or monitor error. The failed first regression scope records
9,157 soft-limit events and zero hard-limit/OOM events. The host OOM-kill counter
remains two across both scopes. These scope observations do not by themselves
establish the absence of userspace oomd intervention; the final audit binds the
unit/journal checks and full attempted-scope memory audit.

The final audit binds all 22 attempted scopes and 75 jobs, including the three
failed scopes above. All scopes are retired and their job intervals do not
overlap. The unchanged guard uses one build job and one test thread, a 4 GiB
soft limit, 6 GiB hard limit and 512 MiB swap limit. Across all scopes the largest
peak is 4,297,060,352 bytes and minimum host available memory is 12,126,171,136
bytes. Sampled host OOM-kill counters remain two, with no new observed OOM
kills or cgroup hard-limit/OOM events. Complete runner endings and unit journals
are retained. Collected successful units no longer have a surviving historical
exit status; these checks do not establish a host-wide absence of userspace
oomd intervention or qualify unrelated processes.

Four development images remain byte-identical to their historical baselines.
The native editor image differs in exactly twelve bytes: four loaded panic
source-line values move by 23 lines after feature-gated declarations were
inserted, and two derived Threshold FNV32 fields change. A fresh matching ELF,
complete-image reconstruction and independent manual review establish this
specific diagnostic change. The `.rodata` section lacks write/execute section
flags, but its containing load segment is RX; no hardware NX claim is made.
The old image SHA-256 is
`a892343cea36f8739d1344d47fd8c5bb312ed5973e7535215d1e2d27b382ab3d`;
the accepted current image is
`e36ceee6a46fb376ac4b22a1c42df166b5985de2dc3ab3c5b21a1cf63b5e5070`.
Whole-image equality is false. The original auditor and its failed baseline
attempt remain retained; `final-audit-v2.json` binds both historical and current
hashes and this exact reviewed exception. No broad metadata masking is used.

## Exact evidence identities

All receipt paths below are relative to
an external validation directory (not distributed).
The complete reviewed artifact manifest binds 21 outputs. Selected artifact
identities are listed below; these are file SHA-256 values from that manifest.

| Artifact | SHA-256 |
| --- | --- |
| `kernel.elf` | `5a05a659e3c63812ab1be19f35e10a71025b55590b83135c4585b446b0fbb80e` |
| `linux-loader-entry-test.img` | `3704b5747426db4e91f9a305f1a46f0f1d7c60437f9cfc75367b64b43f1ed272` |
| `linux-loader-entry/native.elf` | `16fae9a8a415b29cbf15d08183c4b03ed865a3b51d703b54984efba438683a41` |
| `linux-loader-entry/native.disassembly` | `9622bc4e00b6de977accf6243830759dfbb7950543a4d7da0e12e3d5997550a4` |
| `linux-loader-entry/file-source.bin` | `85e64f97e348786a8fb4d9f3d52fec289e2fb86bba20f0731dfe61990525e0f7` |
| `linux-loader-entry/random.bin` | `3c1dfa0772f47d3a21009b699097f104af83c7fb7e186c508b75ac5a928a1599` |

| Receipt or record | SHA-256 |
| --- | --- |
| `qualification-input-v1.json` | `851a08ca815ca6166398884b67b73e40efad9fd548bd9509483f537646611121` |
| `source-freeze-v1.json` | `ebf951a02cc302c6288b77e0c4254d20dc73a55e244f2e081d9e92df98abaae9` |
| `focused-v1-artifact/hashes.json` | `532dee8d9917dacefaf4f0c90a6540a62d27fbef701246e1e976bca58b6a891c` |
| `kernel-emitted-review-v1.json` | `6e6518751d3a1d3a4d710274725c20fe538f000d6132c3c0ab327f05e18c2f20` |
| `kernel-emitted-semantics-v1.json` | `54dee1d2c15ae720317c10bc7efffc4c1ee9862900201fed3a92b07f751aeabc` |
| `kernel-root-crossreview-v1.json` | `3a1f7ed1e27be32d44004bad053756dc60e3883c426f51171501d06e6811fd3b` |
| `native-structure-v1.json` | `6893ef31657b5ef5ffbd9810f57ba352102ce9eb34efef7bffa1ba753548011e` |
| `native-semantics-v1.json` | `91769816a38d06f552dfc7e0185e204bc7b8d677a21431ae1993384b1e7c42ee` |
| `native-crossreview-v1.json` | `a5c17015a930bdfbf05e32ba22be1428765a09f68ddb653f18f9fdaa5df656db` |
| `fresh-native-review-v1.json` | `85fe868f19d0c5886fa0eb7e1ac34ea0be53f3fddbed85cbbb53e3540a0d5d44` |
| `reviewed-image-v1.json` | `366af6942cad1491f83f1a68767e833298898fbc2bfd300cd0524e504df8a47b` |
| `tcg-v1-identity-receipt.json` | `9625f598a91778a587f8d6cca91a1254d2f343046c9e38a3bc44c5978b1884ec` |
| `qualification-v2.json` | `00417cc97738d237e1bad4be7740c9c395900f59df946583c9ce2de18bc154a3` |
| `qualification-v2-results.json` | `a8a406cd8b2f4a59657974d96f22bdcb4feb53d39d96f0e6202b94e01de3c857` |
| `qualification-v2-memory-summary.json` | `5f90b575da654bb748e4ac90ba84f9b8182844a8fc64c9d9b90b00473cfeb6b4` |
| `qualification-v1-results.json` | `cbbf7c4a0afd2292ecb439a22d2b7ebb2c06fef5e77672655d7b16dc2abdda83` |
| `qualification-v1-memory-summary.json` | `8170a53f1045af3f5680e97b477a0733673fc1d24a64f4ec41bc648e1611b7d6` |
| `kvm-v1-results.json` | `84bd74ebb48c36a9403a4dc2f1b2d3e98f48b37700fa79b2e71a9875efd5c449` |
| `kvm-v1-memory-summary.json` | `4201783b2de768ee5565b0434c4e2c03bd4fc4e105e7b4fce4fa678a58bb7118` |
| `negative-v1/receipt.json` | `67db60856ab9170b44774f48831470456a86feaa7c0c30bbfdc948fe35d2f179` |
| `vbox-v1-final/receipt.json` | `3d9efd06b5053f6cb5b4b6eaade4b3ce9d4758906fc555c5af835846afe4b295` |
| `kvm-v2-identity-receipt.json` | `805444f67226a6768e0f8ab1d6606fc77f21c8d978637e471def5ad63493bbd1` |
| `physical-crosscheck-v2-receipt.json` | `bef97a16e04161179cc3efe2d2d7408765c09b2f07224b35b6b901ea897632c5` |
| `memory-audit-final-v2.json` | `6389538174b2a02a9e24cd1248bde060c2d73eac62834564f928d65fa04180b8` |
| `final-audit-v2.json` | `05034e4ef329bb61a54a5a8ee7c6e3987f380f7efa3af7c3d1d2ec27fcc5db7d` |
| `editor-baseline-capture-v1.json` | `6689c6ee7288b0813eeb988f03b3998895d7435b3243f0b4870bb634ab08f76c` |
| `editor-baseline-metadata-v1.json` | `3605abc5aea51f9c19a065f50b0bafe00266c0313216f23c839f3133ac797f57` |
| `editor-baseline-independent-review-v1.json` | `634c6dfa8996d12ed6a66e7a75d8b7369b9010d7a4ff293a55508cbf958afa42` |
| `final-audit-attempt-v1-failure.json` | `5e726046a09e0447036183185c73261378bdc94fdf262e4b37807149ea636b7f` |

## Next boundary

Retain a source-backed mapping beyond the native call and qualify one coherent
ownership/publication commit across the caller's mapping plan, selected root,
circle resources and user-copy validation. Withdrawal and translation
invalidation must precede resource release. Only then may the userspace veil
return a Linux mapping address and observe the unchanged interpreter's next
request. Dynamic protection/remapping, remaining file operations, dependency
loading and TLS/libc startup remain open before a real `true` exit. The target
remains the existing Linux Antimatter/Electron build; this work does not require
a native editor frontend rewrite or impose a global path ceiling.
