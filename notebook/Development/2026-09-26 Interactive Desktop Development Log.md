---
title: "Interactive Desktop Development Log"
date: 2026-09-26
tags: [desktop, boot, renderer, shell, input, privacy, qemu, virtualbox]
status: implemented
domain: development
---

# Interactive desktop development

The user wants a desktop they can keep running and test alongside development.
The selected increment is proposed [ADR 0031](../../docs/adr/0031-add-an-interactive-bootstrap-desktop-development-image.md):
a separate interactive image, rendered through the existing isolated native
source/renderer path, with Enter opening the existing bootstrap shell and
`desktop` returning to the original public frame.

This work preserves the separately implemented host recovery coordinator in
[ADR 0030](../../docs/adr/0030-prove-bounded-presentation-recovery-coordination.md).
It does not deploy that coordinator or the semantic service into a vessel.

## Runtime and tooling

- Run the exact successful native presentation before external IRQ setup;
  preserve a virgin interrupt backend through that explicit returning mode.
- Restore diagnostic framebuffer ownership only after the presenter drops;
  initialize the normal PIT/PS/2 backend once.
- Cache only the verified public desktop aperture in bounded supervisor-only
  storage. Return from the scoped shell before restoring and reading back it.
- Preserve the input decoder across sessions. Recognize `desktop` only in the
  returning development session. Normal boot keeps its five commands.
- Build `build/desktop-dev.img` independently from the normal/proof image.
  Provide `make desktop-run`, `make desktop-smoke`, and `make desktop-virtualbox`.
- Keep native proof precedence in combined-feature builds. Keep launcher
  implementation and markers absent from the normal image.
- Validate the actual boot/native/IRQ/launcher event order; never complete a
  host semantic transaction from the development transcript. Retain only
  public desktop captures after independent exact pixel verification.

## Validation

The first `cargo xtask desktop-smoke` passed with the final kernel input gate.
Its strict 42-record transcript covered native rendering before IRQ setup,
initial launcher readiness, edited `versiom` to `version`, private echo, two
shell visits, and two exact restorations. The child was still running at the
end of the exercise. Both retained 1024x768 desktop captures matched the
independent oracle at every pixel; the restored PPM was byte-identical to the
initial capture. The private input marker was absent from the transcript.
The public desktop capture was also visually inspected.

The development image SHA-256 is
`1374c8b169ba43b3255b843a4a1b0e98c622574a831b24fa18b13525bf2014d5`.
The new tooling tests cover malformed/private/reordered transcripts, each
capture boundary, failure-artifact cleanup, feature precedence, normal-image
exclusion, and bounded diagnostic drain.

VirtualBox was attempted using the desktop mode with a headless frontend.
Image conversion and VM configuration succeeded, but VM startup failed with
`VERR_SVM_IN_USE`: KVM already owns the host AMD-V facility. The VM remained
powered off. No VirtualBox guest boot or framebuffer claim is made, and no host
hypervisor configuration was changed.

The official `make desktop-run` command launched a graphical QEMU window on
this host and reached `desktop_dev.ready` without an error/fatal record. Its
serial file has mode 0600. The rebuilt image retained the same SHA-256 above.
It was left running for interactive user testing while the remaining regression
suite ran. Graphical QEMU serial and stderr are each capped at 64 KiB and
continuously drained; raw guest diagnostics are not echoed to the terminal.

The first full check passed all host tests, including 370 desktop-feature
kernel tests, 370 all-feature kernel tests, and 74 xtask tests. It then caught a
launcher lifetime bug: `desktop-run` retained the shared artifact lock while
QEMU stayed open, preventing subsequent image inspection/build commands.
The correction retains the lock through build, inspection, and creation of a
private per-run disk copy, then releases it before launching QEMU. This also
protects running desktops from a later desktop-image rebuild. Per-run private
serial/stderr logs remain separate across launches.

The corrected graphical launcher was restarted and reached readiness using
its independent disk copy. A focused regression test proves build-lock
exclusion/release, two independent run disks and logs, owner-only modes, and
post-run image cleanup. Review found no remaining launch-lifetime issue.

The fresh full `make check` passed while that corrected desktop stayed live,
including 370 desktop-feature kernel tests, 370 all-feature kernel tests,
75 xtask tests, existing crate tests, formatting, native payload inspections,
combined-feature links, normal-image exclusion, warning-denied Clippy, and
VirtualBox script syntax. The live and stable desktop image hashes remained
unchanged after these builds.

The complete required QEMU matrix then passed: `smoke`, `fault-smoke`,
`circle-smoke`, `control-smoke`, `rite-smoke`, `scheduler-smoke`,
`supervision-smoke`, `shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`,
`input-wait-smoke`, `shell-smoke`, `semantic-surface-transport-smoke`,
`semantic-presentation-smoke`, and final `normal-exclusion`. Additional
`display-vessel-smoke` and `supervised-renderer-smoke` checks passed. The
interactive desktop remained live throughout this fresh regression run.

A final `make desktop-smoke` also passed after the launcher lifetime fix and
full matrix. It again produced exactly 42 allowlisted records, two shell visits,
no private probe in serial, and byte-identical initial/restored public frames.
The retained public preview matches this final capture. The independently
running graphical QEMU continued using its private image with the original
SHA-256 throughout the desktop rebuild, and was left open for the user.

## Remaining boundary

The shell is still kernel-hosted. This launcher has one focused keyboard
activation, no mouse or window lifecycle, no dynamic canonical service, and
no native machine/accessibility action endpoint. Persistent user-space scene
ownership, authenticated cross-process presentation, service supervision and
recovery remain the next desktop boundary.
