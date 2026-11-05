# Workspace application launcher

The workspace launcher is a popup in the existing userspace desktop service.
It opens six public built-ins:
Welcome, System Inspector, Activity, Appearance, Graphics Lab and Editor Lab. It consumes
no window slot. General process launch, Reliquary `.re` loading and native
Antimatter remain separate implementation gates.

## Canonical state and commands

Launcher visibility and the selected `launcher_choice` belong to the same
published snapshot as windows, focus and pointer arming. Human, accessibility
and machine projections observe that state. Typed toggle, dismiss, select and
activate commands use the existing grants, revisions and presentation-then-
publication transaction. Opening resets selection to Welcome without changing
window focus. Selection alone never opens an application.

Activation restores, raises and focuses an existing matching built-in on the
current workspace, or creates it using existing geometry and capacity policy.
It closes the popup in the same transaction. Rejection preserves the entire
committed state. Existing windows remain launchable at full capacity; absent
apps are visibly unavailable. Graphics creation requires a validated surface
descriptor and retained immutable pixels from the trusted service; reopening
preserves existing content and its sequence. Menu labels confer no authority.

## Input and presentation

Applications or F1 toggles the popup. Arrows and Tab/Shift-Tab select entries;
Enter/Space activates. Escape first dismisses the popup; with it closed, the
existing shell-return behavior remains unless an editor owns focus; there,
Escape collapses the text selection. F4 opens or restores Editor Lab. Other application and development
shortcuts do not run while the popup owns keyboard routing.

Pointer controls retain ADR 0036's matching-release contract. An outside press
dismisses the popup and consumes the whole gesture, including its eventual
release. The menu background and disabled entries block underlying controls.
Keyboard input, source loss, extra buttons and stale revisions cancel armed
gestures. No pointer transaction exceeds the existing four-command bound.

The popup paints above windows and uses the same geometry for hit testing.
Selection, availability, hover and press have visible states, including High
Contrast. The bottom panel contains Applications, task buttons, workspaces and
Appearance. The separate theme, density, font, cursor and layout buttons are
removed; Appearance and existing keyboard commands retain those operations.

## Evidence requirements

Model tests cover authority, atomic rejection, capacity, app reuse and retained
Graphics content. Input tests cover routing, cancellation and dismissal without
click-through. Compositor tests independently check geometry, occlusion and
visible state across themes, text sizes and densities. Native smoke expectations
choose semantic commands independently of the input adapter and compare full
frames, including launcher-to-Graphics surface binding, shell return and fresh
session cleanup. VirtualBox rendering is checked separately from QEMU.

Editor Lab is public memory-only scratch content under [ADR 0064](../adr/0064-add-a-public-keyboard-editor-workspace.md).
It is not the independently isolated Antimatter application or a durable editor.
