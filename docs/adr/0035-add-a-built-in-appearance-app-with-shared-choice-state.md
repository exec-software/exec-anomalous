# ADR 0035: Add a built-in Appearance app with shared choice state

- Status: Proposed
- Date: 2026-09-26

## Context

ADRs 0032–0034 establish the bounded native workspace, canonical appearance,
pointer controls, and Clay-inspired frame assets. Global shortcuts and small
panel controls expose those preferences, but do not provide one place to see
the available values or choose a value explicitly. A settings app is a useful
next public-content client of the same model before admitting arbitrary
application processes, buffers, or text.

Expected user mistakes must also remain recoverable. Reaching a geometry or
capacity limit should reject the candidate without ending an otherwise healthy
desktop session. Identity, input-source, grant, and presentation failures still
require withdrawal through the established containment path.

## Decision

### Keep the app inside the existing canonical owner

Add a fixed `appearance` content selector alongside Welcome, System Inspector,
and Activity. It is a built-in app inside the existing workspace vessel, not a
separate process or a general application-surface protocol. It introduces no
private or arbitrary text, kernel rights, external client authentication,
storage, or stable wire format. Existing four-window and two-workspace bounds
remain unchanged; the initial view still contains the same two public windows.

`open_appearance` restores, raises, and focuses the singleton on the active
workspace, retaining its previous selection. If absent, it creates a centered
840×568 window, reduced to fit the canonical work area. It never evicts another
window to create space. Direct creation of a second Appearance window is
rejected. Opening requires both manage and arrange grants, plus the existing
observation requirement. Treat opening as a possible creation in the existing
four-command batch's at-most-one-creation rule, even when a particular opening
would reuse the singleton.

### Share option identity, selection, and activation

Expose one fixed ordered set of sixteen typed choices: three themes, two
densities, two embedded bitmap text scales, two cursor shapes, two frame-corner
profiles, two depth preferences, two reduced-motion preferences, and reset.
Selection has a stable next/previous traversal and lives in the Appearance
window's canonical state. Current setting indicators and the keyboard-selected
choice are distinct observations, shared by human, accessibility, and machine
views. Reset is an action, never an enduring selected setting.

`select_appearance` requires arrange authority and changes selection/focus.
`activate_appearance` requires arrange and customize authority; it focuses the
same visible Appearance window, records the choice, and changes the preference
in one candidate. Both reject stale windows, other content, hidden windows,
and windows outside the active workspace. Revision, endpoint, publication,
revocation, and full-frame acknowledgement rules remain in force.

Component choices preserve unrelated settings and overrides. Rounded and
square frames set a narrow `corner_radius` override to 6 and 0 respectively.
Resolve it after the optional complete frame profile and validate the final
radius within 0–12. Preserve any explicitly authored full frame override
unchanged; without one, later theme changes still choose their own shadow,
border, separator, and control defaults. Clearing the narrow override restores
the selected theme or authored profile's radius. Reset clears all overrides and
replaces the complete settings value with the initial Light configuration. The
existing theme and geometry validators decide whether the candidate can publish.

The read-only enabled predicate copies the snapshot, applies the prospective
settings mapping, and normalizes theme and geometry without mutating the owner
or reserving a transaction. An incompatible retained palette can disable a
theme choice; reset is also disabled when custom workarea changes would strand
windows outside the initial work area. Selecting a disabled choice remains
possible for keyboard inspection, but human activation is ignored. This
predicate confers no authority: grants, visibility and revision still apply
when submitting an action. No focus, selection, preference, or pointer mutation
from a rejected batch may leak into the committed revision.

### Paint and route the same controls

A header button and **P** open Appearance. The mouse chooses explicit settings;
**F6** and **Shift+F6** move the selected choice, and **Enter** or **Space** applies
it. **Tab** remains window traversal; global window movement, layout, and
appearance shortcuts retain their existing meaning.

Painting and picking consume one shared bounded client/control layout. The
topmost opaque window prevents clicks reaching covered choices. Small or tiled
Appearance windows show a maximize prompt when their controls cannot fit;
hidden controls have no pointer targets or keyboard activation. Visible
selected, disabled, hovered, pressed, and keyboard-focus states must remain
readable in Light, Dark, and High Contrast.

Depth preferences and effective behavior remain distinct: High Contrast forces
flat frames while retaining the requested depth value. Reduced Motion is a
session preference; the compositor currently has no animation backend and
always resolves zero animation duration. The UI must describe these limits
without implying nonexistent effects. All settings remain session-local and
are discarded when the owner withdraws; `desktop` constructs a fresh session.

### Contain bounded policy rejection without withdrawing

Limit recoverable native policy errors to invalid geometry, exhausted window
capacity, invalid theme, and a layout too small for its windows. Their candidate
is discarded atomically. A rejected keyboard action keeps the current frame.
A rejected pointer action cancels capture and may publish only the packet's
bounded position and button state through a fresh ordinary transaction, so
release/pressed state cannot stick to a failed action.

Unexpected errors, including source continuity, identity, grant, transaction,
and presentation failures, continue to withdraw the vessel and restore the
bootstrap shell. Do not generalize the recoverable allowlist to every error or
treat presentation uncertainty as an ordinary settings failure.

## Alternatives and reversibility

A separate app process would require surface admission and authenticated
semantic IPC before the current work can exercise settings. A private UI-only
selection or direct renderer mutation would separate human and machine state.
Keeping settings only in global cycle shortcuts would avoid new content but
would leave values and unavailable combinations hard to inspect.

The built-in content and typed choices are removable additions to the existing
model. They establish neither an external app ABI nor a persistent profile
format. General applications, storage-backed settings, font loading, color
editing, login/audio services, and external agents remain separate work.
Accepted fixture ADRs remain unchanged.

## Validation

Require model tests for singleton lifecycle, capacity and viewport bounds,
union grants, stale/hidden/wrong-content rejection, component preservation,
disabled choices, navigation, whole-batch rollback, and equal human/machine
observations. Require adapter and compositor tests for visible control gating,
keyboard navigation, retained global shortcuts, occlusion, readable control
states, exact bounds, and the limited recoverable-error policy.

Extend native smoke through explicit pointer and keyboard settings changes,
singleton restoration and cross-workspace reuse, reset, rejected policy actions,
and fresh shell reentry. Compare complete native public frames with the host
build while keeping independent semantic and directed pixel assertions. Run the
full repository validation matrix and normal-image exclusion before handoff.
Record measured evidence in the notebook separately from this proposal.
