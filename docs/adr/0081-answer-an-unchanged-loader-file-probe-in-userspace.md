# ADR 0081: Answer an unchanged loader file probe in userspace

- Status: Accepted for the bounded loader file-probe proof
- Date: 2026-10-08
- Builds on: ADRs 0053, 0073, 0079 and 0080

## Context

ADR 0080 qualified one real `brk(0)` reply to the unchanged Linux interpreter.
Its next measured request was `access("/etc/ld.so.preload", R_OK)`. The target
remains the existing Linux Antimatter/Electron application, with unchanged
dynamically linked `true` completing startup and exiting as the nearer gate.

The native veil protocol already has operation 18 for copying a suspended
caller's bytes under sealed call authority. The owned execution controller
already preflights inactive mappings and current-veil output. Reuse these
mechanisms; pathname interpretation and Linux namespace answers belong in the
userspace veil, not privileged code.

## Decision

Advance the default-off `linux_loader_entry_test` profile to two actual Linux
returns followed by the third observed SYSCALL and explicit retirement. Keep
the original executable/interpreter, source snapshot, native mapping plan,
Linux initial stack and three-task CPU/TLS ownership. No new native operation,
stable format or general filesystem is introduced.

Version the private 128-byte userspace policy prefix as `ANLPOL02`: role at
offset 0, magic at 8, little-endian version 2 at 16, size 128 at 24, initial
break at 32, namespace kind 1 at 40, pathname length 18 at 48, and the exact
eighteen bytes `/etc/ld.so.preload` at 56. Bytes 74 through 127 are zero.
Namespace kind 1 declares only that this exact name is absent in the initial
personality view. It does not infer host filesystem state or grant lookup,
mapping, credential or storage authority. The host independently constructs and
binds the complete record; the kernel writes only the existing role word.

A feature-gated, allocation-free userspace policy decoder accepts only that
complete declaration. The veil first returns the declared break as before.
For the next access request it validates the Linux integer mode bits and copies
one byte at a time through native operation 18. Stop immediately at NUL, without
requiring a following page to be mapped. Checked address increments cannot wrap.
Only the exact declared absent pathname produces `-ENOENT`. Inaccessible memory
produces `-EFAULT` and invalid mode produces `-EINVAL` in the pure policy.
Unknown paths and exhausted work remain unsupported; there is no synthetic
successful access result. Only the measured valid mode/path is physically
qualified by this profile.

The policy has a 256-byte per-invocation work budget. The controller retains a
256-byte contiguous-copy diagnostic journal for this removable proof. Neither
is a fixed native pathname limit; incremental/resumable lookup remains future
work. Unsupported policy outcomes do not qualify an alternate ending: the
bounded controller requires its expected second return and fails the proof if
that progression cannot occur.

The native read endpoint requires the exact current veil/frame/root/TSS,
nonzero request sequence, zero reserved words and the existing 1–4096-byte
native copy bound. The proof's remaining journal budget is checked before any
copy. Read through the sealed suspended-caller authority into retained 4096-byte
kernel scratch; completely preflight current-veil output before publishing it.
Append the diagnostic journal only after a successful output copy. Volatile
wipe and read back the entire scratch on every returned outcome. Around every
attempt verify all twenty-two saved caller-frame words, copied request and
pending authority remain unchanged. No native read changes caller registers or
chooses a Linux pathname/result.

Six physical negative requests cover zero/stale sequence, zero length, nonzero
reserved argument, inaccessible caller address and a read-only veil destination.
The native veil checks exact failure results and an unchanged writable sentinel.
Nineteen successful one-byte reads include the pathname's terminating NUL.
Complete the second request with an authenticated RAX-only `-ENOENT` reply,
preserving all other saved words and the waiting-veil token. Observe a fresh
third request rather than predicting its number. Retire there with userspace
`-ENOSYS` bits without returning those bits to Linux, then reclaim all owners.

## Qualification and limits

Require hostile host policy, completion/copy-shape and transcript tests, source
review and a fresh emitted entry/return/copy/stack review before booting. The
physical oracle requires fifteen ordered records: eight boot records, ready,
three boundaries, two committed replies and complete retirement. Independently
decode both full before/after frames and require only RAX changed each time.
All three SYSCALL sites must resolve to retained unchanged interpreter bytes.
The second reply also reports the actual copied bytes, address, sequence and
length. Independently bind them to the declared pathname and a unique initialized
read-only/nonexecutable source mapping. Require three raw calls, two returns,
34 native calls, 25 read attempts, nineteen successes, six rejections, cleared
scratch, three retired tasks and exact final source/mapping/TLS/CPU restoration.

Exercise TCG/KVM at 96/320 MiB, malformed source provisioning and VirtualBox under
its existing capability gates. Run all required and affected regressions and
independently audit retained evidence before acceptance. Preserve failed attempts
and capability refusals. Serialize builds and VMs under the unchanged memory
guard; compare all five development image baselines.

Qualification passed on 2026-10-08: TCG and KVM at 96/320 MiB each observed
both actual caller returns and a third request, `openat` for `/etc/ld.so.cache`.
The three malformed source cases, all twenty-nine required/affected regression
targets and final independent evidence audit passed. All five baseline images
remained byte-identical and no OOM was observed. VirtualBox's existing CPU
capability refusal and clean poweroff are recorded separately. The
[Linux Loader Access Log](../../notebook/Development/2026-10-08%20Linux%20Loader%20Access%20Log.md)
retains the exact qualification, artifacts and limits.

This increment does not qualify general namespace traversal, permission checks
on present files, symlinks, descriptors, durable storage, heap growth, dependency
mapping, libc startup, a Linux exit, rendering or Antimatter launch. The private
absence declaration is one explicit userspace fact, not a filesystem substitute.
The next service must follow the next observed loader request.

## References

- [Linux v6.17 access implementation](https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/open.c).
- [glibc 2.42 loader preload probe](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/elf/rtld.c).
