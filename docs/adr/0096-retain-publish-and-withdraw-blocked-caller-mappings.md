# ADR 0096: Retain, publish and withdraw blocked-caller mappings

- Status: Accepted for the bounded inactive-caller proof; hardware use remains unqualified
- Date: 2026-10-15
- Builds on: ADRs 0090, 0091 and 0095

## Decision

Extend the native proof with four feature-private operations. Operation 26
retains the source-backed candidate using operation 25's six geometry words;
its reply supplies a nonzero slot generation and service incarnation. Operations
27, 28 and 29 publish, inspect and withdraw/release that slot. Their arguments
repeat the pending sequence, generation and incarnation, with three reserved
zero words. These numbers correlate requests; genuine retained source, caller,
veil, scheduler authorization, roots and saved frame establish authority.
Operation 25 retains its synchronous preparation/cancellation contract.

The mapping owns the actual source controller by value. Readers and allocator
borrows are lexical, and no self-reference or fabricated static lifetime is
stored. Source recovery is permitted only after mapping cleanup releases every
lease. Retain partial, published and uncertain owners in kernel storage.

The first publication adapter supports one otherwise empty directory-pointer
slot at 4 GiB. Both full trees, leases, geometry and complete virtual exclusions
are checked before the external store. Never replace the caller's PML4 entry.
An explicit ten-mapping circle policy admits the nine original regions and one
runtime extent. The retained slot selects the ten/three caller/veil validator;
observed resource counts and user handles cannot select a weaker policy.

Normal suspended-caller copies compose both owners. Every followed table and
data address derives from retained ownership, with exact permissions and full
range preflight before bytes are exposed. Compare the entire source prefix and
final-page zeros through this path; preserve original copies and reject writes,
absent tail pages and neighboring addresses.

The caller remains blocked throughout this proof. Root selection and foreign
reply commits are forbidden while a candidate is retained. Publication marks
uncertainty before external mutation. Any ambiguous result retains all owners
and prohibits continuation. Withdrawal removes the exact edge, verifies the
complete original tree, and completes translation invalidation before cleanup.
The present contract requires sole BSP, IF clear/NMI masked, and no PCID or global
translations; it does not qualify future SMP shootdowns or caller activation.
Detach the extra circle child and recover the source only after lease cleanup.
Terminal teardown must process the runtime owner before removing its alias or
original launch. Failures cannot discard ownership to make accounting pass.

## Qualification

Require owner, publication, copy, denial, uncertainty and cleanup host tests;
exact native handle/replay probes; fresh emitted kernel/native review including
constructor moves and privileged stack bounds; and serial guarded TCG/KVM boots
at both memory sizes. Preserve the earlier seven file-service lifetimes and the
unanswered sequence-11 Linux mapping request. Run repository and affected
regressions, retain failed attempts, and report VirtualBox separately.

Qualified at code checkpoint `924e62c061505902955a6999aaf93123bce7f5b8`.

Fresh kernel and native emitted reviews, four TCG/KVM boots at 96/320 MiB,
independent physical verification, six provisioning refusals and all thirty
required/affected regression commands pass. The first formatting attempt remains
failed evidence. VirtualBox separately refuses `application_cpu_amd_error_pointers`
and is verified powered off; it has no loader-execution pass. The final audit
records fifteen guarded scopes and sixty attempted jobs, no overlapping guarded
job intervals and no observed OOM kill. Soft-limit reclaim occurred during
`make check`, without a hard-cap event.
The [Mapping Publication Log](../../notebook/Development/2026-10-15%20Mapping%20Publication%20Log.md)
records exact identities, changed baseline images, retained failures and evidence
limits. Acceptance applies to this bounded inactive publication/withdrawal proof.

The caller remains suspended at sequence 11 after ten Linux returns. The
read-only, non-executable candidate is never selected as a running caller
mapping. This slice adds no post-publication guest fault injection and proves
no Linux mmap return, main `true` entry, dynamic linking, Linux exit or Antimatter
execution. Next qualify hardware-use ownership, safe caller activation and the
exact authenticated userspace Linux mmap reply, then observe the next interpreter
request. Foreign syscall and ELF policy remain in userspace.
