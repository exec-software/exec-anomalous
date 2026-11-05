# ADR 0064: Add a public keyboard editor workspace

- Status: Proposed
- Date: 2026-10-05

## Context

The native document proofs establish repeated editing and correlated durable
saves, but they use scripted clients. The workspace already receives copied
physical input and publishes acknowledged complete frames. A first interactive
editing gate can exercise that path before connecting an independently isolated
editor to the document service. Antimatter's Electron/React/CodeMirror frontend
and its Linux behavior reference remain separate from this Rust fixture.

## Decision

Add Editor Lab as the sixth public workspace built-in. F4 or Applications opens
or restores it on the active workspace. Its initial text is empty. Restoration
preserves its text, selection and history; closing discards them. The window
explicitly identifies its text as public, memory-only scratch content. Ctrl+S
reports `save_unavailable`; it cannot initiate or acknowledge a document save.

A new copyable `local_editor_state` owns only local text, baseline, selection,
revision and one immediate undo predecessor. It reuses the existing staged
repeated-insertion validation. Deletion stages its temporary selection and
preserves the original caret/selection in the undo predecessor. Rejected edits
leave local state unchanged. Content no-ops preserve the prior undo entry.
Changed edits replace that entry only after validation and report replacement.
There is no document identity, service request, pending save or invented saved
version. The existing opened models remain non-copyable and retain their APIs.

The canonical desktop snapshot owns this deliberately public scratch state and
structured feedback. A compact owned editor projection exposes text, selection,
caret, local revision, dirty state, history depth and feedback to both human and
machine observers. This is an explicit public built-in content policy; ADR 0038's
exclusion of private document bytes from copyable desktop snapshots still applies
to external applications. Private editor content needs a separate scoped design.

Typed editing commands use the existing observe/present authority and exact
committed desktop revision. They require the current focused, visible editor
incarnation on the active workspace. Stale identity, focus, grant or revision
rejects the entire transaction. Legitimate local edit rejection instead prepares
a canonical feedback update while preserving text and history. Every mapped
editor action follows candidate composition, begin, all acknowledged display
rows, attestation and publication. No local edit becomes committed before that
sequence. Cancellation, failed batches and withdrawn presentation preserve the
previous committed state.

F1 and the launcher retain modal precedence. Outside it, F4 opens/restores the
editor and unmodified or Shift-Tab traverses windows. A focused editor consumes
all remaining keyboard keys: desktop shortcuts never receive its text. The
bounded profile translates US printable ASCII, Shift/Caps letters and shifted
punctuation; arrows/Home/End move the caret, Shift extends selection, Backspace
and Delete edit, Ctrl+A selects all, Ctrl+Z undoes, and Escape collapses selection.
Unsupported keys/chords produce feedback, including development function keys.
Both Control/Alt sides are tracked. Existing exact source/generation/sequence
validation and repeated-make suppression remain in force; this adds no typematic
behavior. Input loss clears held modifiers and Caps state and reports a canonical
input reset, including while the launcher covers the editor. Pointer focus and
window controls remain supported; pointer text selection is a later gate.

The compositor paints directly from the immutable candidate, row by row. The
caret remains visible through deterministic horizontal scrolling at the chosen
text scale. Selected text and caret use checked theme contrast; selection has a contrasting
underline so selected spaces remain visible. A window too
small for the line shows an enlargement hint or rejection feedback and rejects
physical editing with `display_unavailable`, preserving text and history. The
input adapter uses the compositor's same visibility predicate. Machine edits
still follow canonical authority independently of visual geometry. There is no
third RGB buffer, kernel policy change, new mapping, wire version, stable binary
format or privileged operation. The fixture's 31-byte single-line capacity is
explicit local test policy, not a general editor, filesystem or path limit.

## Qualification and disproof

Host tests must establish atomic edits, retained undo, exact authority and focus,
publication/cancellation, lifecycle and input isolation. Independent glyph tests
check pixels rather than only comparing two executions of the same compositor.
The native smoke sends physical keyboard and pointer input, checks literal
semantic expectations independently of the adapter, compares every complete
frame, exercises fresh window incarnation and returns to the shell. Existing
workspace pointer and Graphics traces must remain valid. Emitted code requires a
fresh stack/control-flow review within the existing user and kernel budgets.
Builds and VMs remain serialized under the memory guard; normal exclusion and
all repository-required checks must pass. Evidence belongs in the development
notebook, without treating a shared compositor oracle as independent glyph proof.

This gate fails if text keys reach desktop policy, rejection silently changes
text/history, publication precedes display acknowledgement, stale identities edit
replacement windows, scratch Save claims durability, private content appears in
public snapshots, or resource bounds exceed the admitted native profile.

## Following gates

Connect focused editor input and semantic publication to a separately owned
native editor and its authenticated durable document service. Preserve exact
pending-save identity while typing, and expose success, rejection and uncertain
outcomes through the same human/machine state. Then compare the same application
contract through ELF loading. Porting Antimatter's actual frontend and runtime
remains required for the Antimatter milestone.
