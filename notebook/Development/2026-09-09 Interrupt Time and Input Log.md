---
title: "Interrupt, Time, and Input Log — 2026-09-09"
date: 2026-09-09
tags: [development, kernel, interrupts, time, input, privacy, ai]
status: accepted
domain: foundations
---

# Interrupt, Time, and Input Log — 2026-09-09

## Objective

**Observed and accepted:** establish the smallest replaceable mechanism that can receive
external timer and keyboard work, return safely from an IRQ, preserve ordering
and visible loss, and keep private physical input separate from human or AI
semantic authority.

[ADR 0013](../../docs/adr/0013-prove-bounded-interrupt-time-and-input-delivery.md)
was committed and pushed as proposal checkpoint `1787d1b` before implementation.
The completed evidence below supports its acceptance without promoting the
prototype controller details or feature hooks to stable ABI.

## Design boundary

The accepted proof is x86-64, BSP-only, fixed-capacity, and allocation-free in
IRQ context. A replaceable `legacy_pc` backend owns only the dual 8259 PIC,
PIT channel 0, and 8042 keyboard hardware needed by the current QEMU and
VirtualBox profile. This temporary selection is necessary because `boot_omen`
does not yet provide ACPI/MADT topology; it is not a permanent platform model.

The implementation establishes:

- dedicated returning IRQ assembly and dispatch, separate from fatal exception
  entry and feature-test gates;
- IF held clear while gates, per-source storage, handlers, acknowledgement, and
  fatal fallback are installed, followed by explicit all-line PIC masking before
  controller reconfiguration;
- only timer and keyboard IRQs armed in this slice, with NMI still masked;
- fixed typed storage for record-producing sources, carrying nonwrapping source
  generation and sequence plus an attributable monotonic timebase generation
  and tick;
- explicit loss ranges that cannot be hidden behind queue overwrite;
- typed physical press/release transitions below a separate provisional layout
  adapter; and
- a fatal `cli` plus privacy-safe diagnostics and `hlt` loop for an impossible
  interrupt state, with the static serial record forced onto a fresh line even
  if it preempts foreground output.

The PIT privately advances ordering, not civil or wall-clock time, and does not
broadcast every periodic tick as a global event. This slice uses immutable boot
timebase generation 1: ticks are monotonic and comparable only within that
identity. A future timebase change must allocate a fresh nonwrapping generation,
whose numeric ticks are not directly ordered against another generation.
Reaching a source-generation, sequence, tick, or checked loss-count limit
exhausts the affected mechanism rather than reusing an identity.

## Human and AI authority separation

A physical key transition says what the device reported. It does not authenticate
a person, authorize an operation, or carry semantic intent. Scan bytes,
physical-key records, derived text, and shell content remain private and must
not enter COM1 telemetry.

The bootstrap shell may temporarily consume the physical stream through a
separate US ASCII mapping adapter so existing interactive development can
continue. That adapter remains a bring-up aid, not a stable input ABI or
Canticle decision.

Future AI control belongs over authenticated IPC and versioned invocations. An
AI client must never fake the keyboard or have its actions attributed as
physical input. Human UI, shell, accessibility, remote-input, and machine
clients can later share semantic operations and policy while retaining distinct
typed origins and audit provenance. COM1 remains diagnostic output, never a
command or authorization channel.

## Acceptance evidence

| Boundary | Observed evidence |
| --- | --- |
| Entry and return | QEMU and VirtualBox continued after real PIT and keyboard IRQs; a feature assembly probe proved general-register and flags preservation |
| Controller ownership | Both feature transcripts reported initial PIC masks `0xff`/`0xff` and active masks `0xfc`/`0xff`; host tests passed for PIC writes, selected masks, spurious paths, EOI order, the exact PIT sequence `43:34`, `40:9c`, `40:2e`, bounded 8042 initialization and degradation, fresh empty-IRQ streak after successful setup/rearm, and decoder state |
| Ordering | Both emulators reported source generation 1, exact sequences 0 through 23, 24 transitions, and monotonic same-timebase observations |
| Loss and rearm | Both emulators reported one exact generation-1 loss at sequence 88 with count 1, IRQ1 containment at master mask `0xfe`, bounded 8042/device reinitialization, generation 1-to-2 rearm at master mask `0xfc`, and post-rearm generation-2 sequences 0 through 1 |
| Input meaning | Tests keep physical make/break decoding private and independent of the provisional shell layout adapter |
| Privacy | The strict COM1 verifier rejects every nonempty unstructured line, malformed record, unexpected event, unknown field, and the injected `privateprobe`; neither emulator transcript contained it |
| AI separation | Normal code has no serial-input or synthetic-key publication path; future AI entry remains authenticated IPC/invocations only |
| Isolation | `make normal-exclusion` passed after rejecting every named feature-only entry, loss, timer-health, counter, and return-probe helper, the `kernel.interrupt_test.` event namespace, and the injected privacy probe in the normal image |
| Regression | `make check`, `make smoke`, `make fault-smoke`, `make circle-smoke`, `make memory-smoke`, `make interrupt-smoke`, `make shell-smoke`, and `make normal-exclusion` all passed on 2026-09-09 |

The bounded QEMU transcript recorded PIT start tick 0 and end tick 8, eight
interrupts, 24 generation-1 transitions at sequences 0 through 23, exact loss
sequence 88 with count 1, and post-rearm generation-2 sequences 0 through 1.
It recorded initial masks 255/255, active masks 252/255, containment at master
mask 254, rearm at master mask 252, and final masks 255/255. The 512 MiB
VirtualBox feature transcript reported 130,674 free frames, start tick 1 and end
tick 9, and otherwise reproduced the same lifecycle with no private probe,
error, or fatal record.

Independent final review found and closed six boundary gaps before those final
runs: fatal records now force a serial record boundary; transcript validation is
complete rather than marker-only; successful setup/rearm starts a fresh
empty-IRQ streak; steady-state `00`, `ee`, `fa`, `fc`, `fd`, `fe`, and `ff`
force explicit loss/rearm while ambiguous `aa` remains left-Shift release; the
exact PIT port sequence is host-tested; and normal-image inspection names every
feature-only helper. The QEMU and VirtualBox evidence above was regenerated only
after that hardened tree was complete.

Earlier separated VirtualBox text and scan-code injection attempts expired the
guest's bounded input deadline because orchestration gaps delayed delivery; each
terminated safely with the static reason `input_timeout`, and no uncertain
execution resumed. The successful feature rerun used explicit
`keyboardputscancode` make/break injection inside the same bounded launch/wait
transaction and passed the full lifecycle.

Afterward the ordinary image was rebuilt and restored. It reported 130,683 free
frames, interrupt readiness, and shell readiness. Injection entered `versiom`,
then Backspace and `n`; the shell accepted the corrected `version` through IRQ1,
completed sequence 1 successfully, emitted the next prompt, and remains running
in `Anomalous Dev [Threshold]`. The fresh QEMU shell gate also corrected that
edited command and passed privacy-safe `echo` and unknown-command checks.
Ignored local evidence is preserved in `build/qemu_interrupt.log`,
`build/virtualbox-interrupt.log`, `build/virtualbox-normal.log`, and
`build/qemu_shell.log`.

## Explicit non-goals

This bounded proof does not include:

- ACPI/MADT, APIC/x2APIC, MSI, multiprocessor routing, or NMI enablement;
- HPET/TSC calibration, wall-clock time, deadlines, or general timers;
- threads, task scheduling, preemption policy, or blocking waits;
- a stable kernel ABI, syscall surface, IPC transport, or user-space driver;
- full layouts, Unicode, composition, shortcuts, mouse, touch, accessibility,
  remote input, or secure attention;
- person authentication from a keyboard event;
- AI control, exec-hag, Canticle, shell authority, or permission policy;
- serial input or any hidden debug control surface; or
- production hardware breadth, performance, or recovery claims.

## Resolved implementation choices

- PIT channel 0 uses input 1,193,182 Hz and divisor 11,932, approximately 100 Hz;
  the host test asserts command write `43:34` followed by divisor writes
  `40:9c`, `40:2e`.
- Physical input owns a fixed 64-entry queue; only the feature proof can force
  deterministic pressure.
- The normal shell drains bounded deliveries and uses the architecture-owned
  interrupt-safe readiness check plus `hlt` while idle.
- Loss clears decoder, provisional modifier, and unfinished shell-line state.
  Explicit rearm first performs bounded architecture-owned 8042/device
  reinitialization, then advances the source generation and restarts its sequence
  at zero; exhaustion remains terminal instead of reusing identity.
- Successful initial setup and rearm reset the empty-keyboard-IRQ streak before
  IRQ1 is unmasked, so a recovered source receives a fresh bounded allowance.
- Steady-state bytes `00`, `ee`, `fa`, `fc`, `fd`, `fe`, and `ff` clear partial
  framing and require explicit loss/rearm rather than becoming physical keys;
  ambiguous `aa` remains an ordinary left-Shift release.
