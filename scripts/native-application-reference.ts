import { EditorState, Compartment } from '@codemirror/state';
import { history, undo, undoDepth } from '@codemirror/commands';
import { EditorView } from '@codemirror/view';
import { editorAppearanceFor } from './renderer/src/environment/highlight.ts';
import { registerEditor, insertAtCursor } from './renderer/src/environment/editorBridge.ts';

const dark = { theme: 'dark' as const, secondaryAccent: '#33b7a6' };
const light = { theme: 'light' as const, secondaryAccent: '#176f83' };
const appearance = new Compartment();
let state = EditorState.create({
    doc: 'const before = 1;',
    extensions: [history(), appearance.of(editorAppearanceFor(dark))],
});
const saved = state.doc.toString();
let revision = 1;
let previous = saved;
let focus_calls = 0;
// Only the production bridge's state/dispatch/focus contract is modeled here;
// this trace does not mount Electron, a DOM, or the complete Antimatter UI.
const view = {
    get state() { return state; },
    dispatch(spec: any) { state = spec.state ? spec.state : state.update(spec).state; },
    focus() { focus_calls++; },
};
registerEditor(view as unknown as EditorView);
const frames: any[] = [];
function capture() {
    const text = state.doc.toString();
    if (text !== previous) revision++;
    previous = text;
    frames.push({
        sequence: frames.length + 1,
        document_revision: revision,
        text,
        anchor: state.selection.main.anchor,
        head: state.selection.main.head,
        dirty: text !== saved,
        theme: state.facet(EditorView.darkTheme) ? 1 : 0,
        undo_depth: undoDepth(state),
    });
}
capture();
state = state.update({ selection: { anchor: 6, head: 12 } }).state;
capture();
if (!insertAtCursor('after')) throw Error('production insertion failed');
capture();
state = state.update({ effects: appearance.reconfigure(editorAppearanceFor(light)) }).state;
capture();
state = state.update({ effects: appearance.reconfigure(editorAppearanceFor(dark)) }).state;
capture();
state = state.update({ selection: { anchor: 8 } }).state;
capture();
if (!undo(view as unknown as EditorView)) throw Error('CodeMirror undo failed');
capture();
registerEditor(null);
console.log(JSON.stringify({ profile: 'antimatter_native_editor_v1', focus_calls, frames }, null, 2));
