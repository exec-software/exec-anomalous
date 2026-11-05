# ADR 0097: Resume a caller with a retained source mapping

- Status: Accepted; bounded retained-mapping resume and terminal cleanup qualified
- Date: 2026-10-18
- Builds on: ADRs 0090, 0091 and 0096

## Decision

Extend the bounded Linux interpreter proof to return its exact first libc
mapping address through the existing native completion operation. Keep Linux
syscall interpretation in the userspace veil. The kernel authenticates native
ownership, source, mapping geometry, pending caller and reply effects.

Publication and inspection do not permit execution. A separate one-use resume
ticket binds the creation sequence, sealed scheduler correlation, caller
identity, root, incarnation and exact result address. Admit the complete
composed mapping while the caller remains blocked and an independent root is
current. Commit the ticket after the prepared scheduler reply and RAX-only
frame update, before selecting the caller. A mismatched commit cannot select a
root or discard retained ownership. Refuse premature, altered and replayed
completions without advancing the pending request.

Hardware admission belongs to the graft, not the candidate's private root.
Hardware may set Accessed on the exact installed parent and the candidate's
directory and present leaf entries. This read-only, non-executable candidate
never admits Dirty, or changes to its unused private PML4/PDPT ancestry.
Complete tree verification compares expected owner-derived entries with only
those permitted hardware additions. Retain every source and mapping lease
through admission, execution, faults and uncertain withdrawal.

The actual retained owner selects the ten-mapping caller admission. Current
copies bind that exact caller and current root; suspended copies use the new
sealed scheduler authorization. Creation handles remain bound to their original
invocation and cannot control the mapping from the next request. This proof
captures that next real request and retires without applying its Linux policy.
Timer entry returns the same current frame; caller root selection is only from
an independent root. Same-root reselection is outside this contract.

At terminal bootstrap continuation, revoke selection, withdraw the exact edge,
validate the original complete tree and invalidate translations before cleanup.
Hardware A/D changes in the original caller tree are validated against its
owned permissions; a raw pre-execution fingerprint is no longer an equality
gate. Fingerprints remain diagnostics rather than authority. Release the source
and mapping only after all consumers are quiescent, both original and runtime
trees are accounted for, and the extra circle attachment is removed. The
existing sole-BSP, IF-clear/NMI-masked, no-PCID/no-global contract remains.

## Qualification

Code checkpoint `aa4b409` passes owner/copy tests, fresh kernel and native emitted
review, and TCG/KVM at 96/320 MiB. All four transcripts contain eight lifetimes
and 234 records. Independent verification and 22 malformed-transcript cases
check complete frames, copies, mapping geometry, service state and call counts.
Six provisioning refusals and all thirty repository/affected make commands pass.

The exact first libc mmap reply returns `0x100000000`. Normal retirement and the
new exact veil UD2 both follow actual interpreter resumption and fresh sequence
12 capture. Premature, altered, stale-sequence and replay completions are refused;
creation handles cannot control the retained mapping from that new invocation.
The six previous file-service fault lifetimes remain covered. The resumed fault
contains the dependent pair before peer completion and terminal mapping/source
cleanup, preserving the pending caller and its complete frame.

Sequence 12 is another Linux `mmap`, requesting address `0x100028000`, length
`0x198000`, protection `5` (`READ | EXEC`), flags `0x812`
(`PRIVATE | FIXED | DENYWRITE`), descriptor `3` and offset `0x28000`. The proof
captures this request without applying its policy or returning it to the caller.
Linux syscall interpretation remains in userspace.

VirtualBox separately refuses `application_cpu_amd_error_pointers` and is verified
powered off; this is not a loader-execution pass. Three changed development
baseline images have substantial kernel differences and are not classified as
metadata-only. Both exact additional workspace runtime checks pass, and the
explicit baseline review accepts bounded regression coverage without claiming
binary or stack equivalence. The final audit accepts the current evidence across
22 retired guarded scopes and 75 attempted jobs, retaining all six failed scopes.
Observed OOM deltas are zero within each recorded boot/scope; the intervening
reboot limits any wider claim. The
[Mapping Resume Log](../../notebook/Development/2026-10-18%20Mapping%20Resume%20Log.md)
records current artifacts, review limits, retained failed attempts, baseline
comparisons and separate boot-local memory evidence.

Returning one read-only libc mapping does not establish dynamic linking, main
`true` entry, Linux exit, executable remapping or Antimatter execution. The
observed sequence 12 request determines the next bounded Linux policy slice.

Reaching the fresh request proves actual root selection and resumed interpreter
execution. It does not by itself prove that a candidate leaf acquired Accessed;
the interpreter may proceed using previously read headers. Hardware-bit mutation
tests and observed guest execution are distinct evidence.
