---
title: "Supervised Semantic Renderer Composition Log"
date: 2026-09-19
tags: [development, desktop, renderer, supervision, privacy, security, ring3]
status: accepted
domain: semantic-control
---

# Supervised Semantic Renderer Composition Log

## Accepted composition

ADR 0026 now accepts the bounded bridge between ADR 0025's canonical scene and
ADR 0024's copied presentation path. A build-time conformance step derives a
private, redacted render capsule from the actual eight-node scene fixture. A
separately linked, fixed-address, freestanding x86-64 `no_std` renderer consumes
that capsule without moving scene ownership, protected source, framebuffer
authority, or presentation policy into ring 3.

The target ELF's sole RX load segment is 6,469 bytes and spans two image pages.
The task has three logical mappings: bounded RX image, one RW+NX Rite mailbox,
and a four-page RW+NX stack. Its only grant is `display_update | task_exit`; the
framebuffer and supervisor-owned RGB8 stage are not mapped into its address
space.

## Native lifecycle evidence

Generation 1 passed one controlled real PIT preemption, staged rows 0 through
383 in strict order, and reached the exact linked `ud2`. The kernel contained
that `#UD`, consumed the supervisor observation, and fully withdrew and cleared
the generation's channel, address space, image, mailbox, stacks, scratch, task,
circle, and incomplete 576-page stage. The live framebuffer remained unchanged.

The persistent supervisor then authorized one fresh generation with new task,
circle, channel, sequence, and stage identity. Generation 2 staged all 768 rows
and exited terminally through the Rite gate without a reply or return to dead
task state. Only after the successful observation and complete prepublication
validation did the kernel perform one full-frame commit. An independent
1024x768 oracle matched all 786,432 captured pixels.

Every tracked frame, lease, mapping, channel, task, circle, authority,
supervision, scheduler, context, CR3, privilege-stack, and task-switched datum
returned to its exact initial state on success. Publication makes every
selection, geometry, ownership, and copy-shaping decision before the first
framebuffer write. A hypothetical post-copy stage-clear failure remains a
terminal quarantine path and cannot emit success or resource-restoration
evidence.

## Privacy and reproducibility evidence

Two builds that differed only in protected source produced byte-identical
complete target ELFs. Their capsules, renderer behavior, and final pixels are
therefore noninterfering for the protected value represented by this fixture.
Feature-owned transcript records are private-content-free, mutable generation
state is cleared before reuse, and the normal image excludes the feature
namespace and code, the private source sentinels, and the renderer glyph
fingerprint.

## Acceptance evidence

Reviewed implementation checkpoint `562d5a4` passed `make check`, including
329 all-feature kernel tests, 56 `xtask` tests, 8 renderer-base tests, and 9
scene-conformance tests. The full established non-VirtualBox QEMU matrix
passed, including strict renderer lifecycle, framebuffer capture/oracle
comparison, all-feature linking, regressions, and normal-image exclusion.
The final strict run freshly observed 65,099 free frames after release and
accepted only the exact canonical 21-record transcript. VirtualBox was not
used for this acceptance slice.

## Claim boundary

This is a removable, fixed-fixture composition proof. It is not yet a
compositor, window server, display manager, live scene service, general scene
IPC protocol, stable graphics ABI, GPU path, focus/input policy, or normal-image
desktop runtime. It proves that one protected canonical scene can cross a
redacted build-time boundary, survive supervised renderer failure and
destructive replacement, and publish exactly one complete verified frame.
