# ADR 0036: Activate desktop controls on pointer release

- Status: Proposed
- Date: 2026-09-26

## Context

The native workspace and Appearance app are useful for interactive development,
but their buttons currently act on mouse-down. A person cannot press Close,
move away, and cancel. Pressed paint also follows a held button into unrelated
controls because it has no recorded press identity. This is a bounded desktop
interaction correction before returning to kernel mechanisms.

## Decision

Move the existing fixed control identities into the public workspace model as
`desktop_target`; the compositor retains its `chrome_target` alias. Record an
optional armed target in each canonical snapshot. Human, accessibility, and
machine views observe the same target and revision. This public feedback adds
no action authority, private content, wire format, process, or kernel ABI.

`arm_pointer` requires observe and arrange authority, a visible pointer with
only its left button down, and a valid current control identity. Non-control
surfaces cannot be armed. Window controls require the exact live incarnation
and applicable workspace/visibility; task controls may restore a minimized
window. Appearance choices require valid content and settings. A separate
`cancel_pointer` command clears feedback without inventing a physical release.
Activation still uses the ordinary command and its full manage/customize or
arrange grants, revision checks, validation, and presentation acknowledgment.

Every prepared successor starts with no armed target. A valid physical
continuation must explicitly rearm it in that transaction. A non-pointer effect
also clears preceding armed feedback within the same batch. Failed candidates
leave the committed snapshot and transaction sequence untouched. No candidate
feedback becomes observable until its complete frame is acknowledged.

The user-space pointer adapter records the pressed control and expected next
revision. Button press publishes position, physical button bits, and armed
feedback without activating the control. Held motion can leave and return to
that original control; it cannot transfer the press to another control. Release
activates once only if the target, canonical arm, revision, and topmost enabled
hit still match. Releasing elsewhere cancels. Pressing outside controls and
moving into one while held does not arm or activate it. Extra-button chords
cancel and require all buttons up before a new gesture. Focus, title dragging,
and resize capture retain their immediate behavior.
A fresh pointer source also requires an all-buttons-up observation, so holding
a button across desktop entry cannot manufacture a new press.

Pointer loss, accepted keyboard makes, keyboard loss, competing revisions,
invalidated targets, and rejected policy actions cancel capture. An unmapped
key or rejected keyboard command must still present canonical cancellation if
an arm was visible. When no arm exists, it creates no unnecessary cancellation
frame. Source, identity, authority, and presentation failures retain fail-closed
withdrawal. Expected pointer policy rejection can publish only position and
buttons through a fresh transaction, which necessarily clears armed feedback.

All packets remain within the existing four-command batch bound: two physical
facts plus one arm, or two facts plus up to two release action commands. The
compositor paints a pressed state only for the hovered enabled target when it
matches the canonical arm and the physical button mask is exactly one.

## Validation and checkpoint

Require independent model tests for grants, shared observation, explicit
continuation, invalid identities, atomic rollback, and cancellation. Require
adapter tests for release, leave/reenter, no press transfer, chords, loss,
keyboard cancellation, and stale revisions. Require pixel assertions across
Light, Dark, and High Contrast for genuine and false presses on all controls.

Extend the native exact-frame oracle with explicit press/release and cancelled
gestures without using the adapter as its own oracle. Preserve the existing
window/Appearance exercise, shell reentry, contained failure, reclamation, and
normal-image exclusion checks. Run the complete repository validation matrix.
Record measured results in the notebook separately from this proposal.

This closes a development-desktop usability checkpoint. Durable profiles,
private app surfaces, text input, external-agent IPC, scalable fonts, animation,
and GPU composition remain later desktop boundaries. Subsequent foundational
kernel work should be selected from the existing memory, task, wait/wake,
interrupt, and supervised-service gaps rather than treating this UI checkpoint
as evidence of a production desktop or general application runtime.
