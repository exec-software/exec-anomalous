# ADR 0052: Prove Linux startup and record-device syscalls

- Status: Proposed
- Date: 2026-10-04
- Qualification: Bounded proof complete on 2026-10-04; all 37 matrix jobs and two restoration jobs passed on 313 unchanged frozen source files. This ADR remains Proposed and establishes no stable ABI or general Linux compatibility.

## Context

[ADR 0051](0051-prove-userspace-elf-loading-against-native.md) qualified an
unprivileged ELF loader with native INT f4 semantics. The next distinct boundary
is Linux x86-64 startup and real SYSCALL entry, compared against the same native
editor/service behavior. ELF admission alone does not provide that personality.

[ADR 0008](0008-isolate-foreign-platform-compatibility.md) and the project vision
keep foreign-platform policy in userspace veils or VMs. This increment is an
explicitly feature-private proof-controller bridge over native mechanisms. Its
fixed syscall numbers, descriptors and status mapping are not a production
kernel compatibility layer. Moving foreign policy into a supervised userspace
veil remains a separate gate before this proof becomes an application service.

## Decision

Run one inspected Linux producer with the existing native INT f4 service. Reuse
the independent editor contract, copied channels, retained task/mapping owners,
eager x87/SSE2 state, fault containment and software presentation path. Compare
the same seven editor states and six input commands, then a separate queued
producer UD2 fault with native peer continuation and complete revocation.
Only two application tasks need be live at once.

The admitted artifact remains a bounded static sectionless ELF64 ET_EXEC with
exactly two LOADs: entry/RX at 0x400000, at most 32 KiB, and RW at 0x800000,
at most 8 KiB. There is no interpreter, dynamic linking, TLS, AVX/XSAVE, libc,
signals, process creation, filesystem namespace, general descriptor allocation,
or arbitrary executable admission. Initialized data, BSS and page slack retain
the existing copy/readback/zero-fill checks.

### Initial process state and entry

Construct the initial record in the existing privately owned user stack. RSP
is 16-byte aligned and points to 64-bit argc=2, followed by pointers to
`anomalous-editor` and `normal` or `fault`, an argv NULL, an empty envp NULL,
and auxiliary pairs AT_PAGESZ=4096, AT_ENTRY=0x400000, AT_NULL=0. Strings are
bounded and NUL terminated; every advertised pointer refers to that task's
owned mapping. RDX is zero because there is no interpreter finalizer. Initial
register/FP state and the stack contents are checked independently of the
client's own startup checks. This is a declared freestanding subset of the
[x86-64 initial-state ABI](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/low-level-sys-info.tex),
not the complete environment produced by Linux's
[create_elf_tables](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/binfmt_elf.c).

Do not invent AT_PHDR: deployment headers are outside both LOADs, so an address
near the RX entry would point into instructions. Do not advertise random bytes,
a vDSO, credentials or CPU capability vectors that this profile has not
established. The two admitted auxiliary entries are sufficient for this client;
stock libc startup is outside the claim.

A retained session owns EFER.SCE, STAR, LSTAR and FMASK, with complete original
values captured before mutation and verified after restoration. SYSCALL does
not change RSP. Entry saves the user RSP only to supervisor scratch and selects
the current task's retained supervisor stack before any push or call. It builds
the existing 176-byte frame, preserving the hardware RCX/R11 return values;
checked IRETQ returns avoid a new SYSRET boundary. FMASK excludes interrupt,
direction, trap and other unsafe entry flags. FS/GS/KERNEL_GS remain zero, and
this sole-BSP, NMI-excluded profile uses no SWAPGS. SCE is enabled only for the
selected Linux producer and disabled for the native peer. These constraints
follow the architectural concerns described by Linux's
[64-bit entry implementation](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/entry/entry_64.S).

### Device contract

Use Linux x86-64 register placement and raw signed-negative errno results for
read=0, write=1, poll=7, sched_yield=24 and exit=60, as listed in the
[Linux syscall table](https://raw.githubusercontent.com/torvalds/linux/v6.17/arch/x86/entry/syscalls/syscall_64.tbl).
Syscall numbers are normalized through signed low 32 bits before dispatch,
matching the Linux x86-64 entry convention; the client exercises this with
`0x100000000` selecting read. Unsupported numbers return -ENOSYS. No libc errno
storage is implied. FD and poll count use Linux's 32-bit argument widths; pointers and I/O sizes remain
64-bit. These descriptors are **Anomalous nonblocking record devices, not
Linux pipes**:

| Descriptor | Direction | Meaning |
| --- | --- | --- |
| 3 | write only | One copied surface record to the native service |
| 4 | read only | One copied input record from the native service |

The exact current producer identity authorizes the fixed descriptor binding;
the number alone is not authority. Records remain opaque to the kernel.
Wrong/unopened descriptors or wrong direction return -EBADF. A valid-direction
zero-length operation returns zero without dereferencing its pointer or changing
the channel. Positive lengths above 4096 return -EMSGSIZE. These checks precede
channel-state handling.

A write publishes one complete copied record or none. Empty permits submission;
queued or delivered returns -EAGAIN; closed returns -ENODEV; an exhausted
sequence returns -EOVERFLOW. A read of a queued record returns its complete
length and atomically releases transport backpressure. An undersized buffer
returns -EMSGSIZE, empty or delivered returns -EAGAIN, and closed returns EOF.
There are no partial records. Failed copies return -EFAULT without publication,
acknowledgement, or partial destination writes. Generic `consume_with` commits
queue clearing without a second fallible operation after a successful checked
copy; native explicit delivery/acknowledgement remains available. Linux's
[VFS implementation](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/read_write.c)
dispatches I/O to file operations; these packet and revocation semantics are
the declared Anomalous device policy, not universal Linux read/write behavior.

Poll supports timeout zero and at most eight 8-byte `pollfd` entries. Other
timeouts or larger arrays return the profile's -EINVAL. Zero entries return zero
without touching the pointer. Negative descriptors are ignored; unopened ones
report POLLNVAL. Unsupported requested event bits are ignored. Results are:

| Route state | FD 3 output | FD 4 input |
| --- | --- | --- |
| empty | requested POLLOUT, unless sequence exhausted | none |
| queued | none | requested POLLIN |
| delivered | none | none |
| closed | POLLERR and POLLHUP | POLLHUP |

Open exhausted output reports POLLERR. Error/hangup/invalid bits do not depend
on the requested mask. Return the number of entries with nonzero revents,
including duplicates separately. Snapshot the complete input, compute results
locally, and preflight the complete copyback before changing any byte. EFAULT
therefore preserves all revents. Linux's
[poll implementation](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/select.c)
defines the filtering/counting conventions; whole-array failure atomicity is a
stronger bounded guarantee here, not a promise made by Linux poll generally.

Yield returns zero and may dispatch the admitted peer, without claiming Linux
scheduling policies or fairness. Exit never returns and interprets the low eight
status bits. This fixture requires zero; a nonzero status is a structured proof
failure. Successful exit retires the sole calling task through owner cleanup.
It does not imply exit_group, signals or wait support. See
[sched_yield](https://man7.org/linux/man-pages/man2/sched_yield.2.html) and
[Linux exit](https://raw.githubusercontent.com/torvalds/linux/v6.17/kernel/exit.c).

## Validation and limits

All 37 final matrix jobs and both image-restoration jobs passed on 313 unchanged
frozen source files. The full check passed 469 Linux-profile kernel tests, 503
all-feature kernel tests, 149 xtask tests and nine shared ELF parser tests.
TCG/Nehalem and actual AMD-host KVM passed at 96 and 320 MiB plus separate
captures. Their proof kernels/images and whole PPM captures are byte-identical.
Each boot validates 25 records, including normal/fault totals of 132/12 calls,
exact syscall/device rejections, seven editor states, six commands, pixels,
queued-fault cleanup, and full task/channel/CPU/MSR restoration. Emitted-code
review binds the real parser, two SYSCALL sites, trusted-stack ordering and
bounded stack estimates to the final artifacts. Normal and workspace images
were restored unchanged. Threshold is unchanged; VirtualBox was not rerun.

The final matrix peaked at 2,480,099,328 bytes (2.310 GiB) within the two-job,
4-GiB soft/6-GiB hard envelope, with every scope event counter zero and no host
OOM kills. Earlier failed check attempts are retained separately and are not
counted as qualification. The
[Linux Application Log](../../notebook/Development/2026-10-04%20Linux%20Application%20Log.md)
records exact hashes, measurements, retries, coverage and the remaining
supervised-userspace-veil gate.

The proof must report its own syscall/device evidence separately from the
native comparison. Hardware timing may vary; semantic records and pixels must
not. No result establishes Electron/Antimatter runtime support, general POSIX,
Linux application compatibility, production security, or Windows/macOS support.
Ny and HAG's accepted direction and independent qualification gates are unchanged.
