---
title: "Bootstrap Shell Log — 2026-09-08"
date: 2026-09-08
tags: [development, shell, input, qemu, virtualbox, privacy]
status: complete
domain: foundations
---

# Bootstrap Shell Log — 2026-09-08

## Intent

Build the smallest honest interactive environment that improves kernel bring-up
without presenting a ring-0 loop as the future secure shell or freezing the
proposed Canticle language prematurely.

## Implemented

- Added a polling PS/2 scan-code-set-1 decoder with Shift and Caps Lock state,
  auxiliary/error-byte rejection, and host tests.
- Added a fixed 128-byte printable-ASCII editor with backspace, visible overflow
  feedback, and best-effort volatile buffer erasure.
- Added framebuffer and VGA-text terminal backends with scrolling and a complete
  printable-ASCII bitmap font.
- Added the read-only `help`, `version`, `about`, `clear`, and `echo` commands.
- Added structured `bootstrap_shell.*` events. Raw command text, unknown command
  names, and `echo` content are not serialized.
- Added an automated QEMU shell test that injects keyboard input and treats
  private-marker disclosure as a failure.
- Proposed Canticle in ADR 0010 as a typed, capability-explicit language whose
  real runtime belongs in an unprivileged vessel.

## Evidence

| Check | Observed result | Artifact or command |
| --- | --- | --- |
| Source and unit checks | 3 ABI, 7 kernel, and 3 xtask tests passed; target Clippy passed with warnings denied | `make check` |
| Threshold boot | Test kernel reached `boot.ready` | `make smoke` |
| QEMU shell path | Injected `version`, private `echo`, and unknown input; expected result events appeared and `privateprobe` did not appear in serial | `make shell-smoke` |
| VirtualBox shell path | VM reached `bootstrap_shell.ready`; all five built-ins, edited input, and unknown input completed as sequences 1–7 | `build/virtualbox.log` |
| Human projection | Commands, literal echo output, unknown-command recovery, clear behavior, and the next prompt were legible at 1024 by 768 | `build/virtualbox_shell.png`, `build/virtualbox_shell_clear.png` |

Generated logs and screenshots are ignored build artifacts. The reproducible
commands and structured events, rather than the local files alone, are the
evidence contract.

## Security and privacy boundary

**Observed:** bounds, static command classification, redacted telemetry, and
volatile editor erasure work on the tested paths.

**Not claimed:** isolation, authorization, secret storage, secure deletion from
all caches or VM snapshots, input authenticity, or production security. The
interpreter and device code execute in ring 0. It accepts no privileged or
externally effectful commands and must not grow into the Canticle runtime.

## Next proof

Keep advancing kernel foundations: diagnostic exceptions and IDT, memory
ownership, time/interrupts, circles, bounded IPC, and vessel launch. In parallel,
compare at least two Canticle syntax/semantic sketches against the notebook task
corpus before implementing the host-side lexer, formatter, and canonical AST.
