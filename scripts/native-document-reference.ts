import { actions, active, isDirty } from './renderer/src/environment/store.ts';

// Exercise Antimatter's production store with a controlled asynchronous I/O
// boundary. This writes no host file and does not mount Electron or a DOM.
const path = 'proof.ny';
let stored = 'const before = 1;';
let version = 1;
let pending: { text: string; resolve: (value: any) => void; reject: (error: Error) => void } | null = null;
(globalThis as any).window = { electronAPI: { environment: {
    read: async (requested: string) => {
        if (requested !== path) throw Error('unexpected path');
        return { path, text: stored, modified: version, binary: false };
    },
    write: (requested: string, text: string) => {
        if (requested !== path || pending) throw Error('unexpected write');
        return new Promise((resolve, reject) => { pending = { text, resolve, reject }; });
    },
    suggestCommand: async () => '',
} } };

function check(condition: boolean) {
    if (!condition) throw Error('Antimatter document reference mismatch');
}
const frames: any[] = [];
function capture(expected: string, saved: string, dirty: boolean) {
    const file = active();
    check(file !== null && file.text === expected && file.saved === saved && isDirty(file) === dirty);
    frames.push({ text: file!.text, saved: file!.saved, dirty: isDirty(file!) });
}
function commit() {
    check(pending !== null);
    const write = pending!;
    pending = null;
    stored = write.text;
    version++;
    write.resolve({ path, text: stored, modified: version, binary: false });
}

(globalThis as any).reference = (async () => {
    await actions.openFile(path);
    capture('const before = 1;', 'const before = 1;', false);
    actions.edit(path, 'const after = 1;');
    const first = actions.save(path);
    check(pending?.text === 'const after = 1;');
    actions.edit(path, 'const later = 1;');
    commit();
    check(await first);
    capture('const later = 1;', 'const after = 1;', true);
    const second = actions.save(path);
    commit();
    check(await second);
    capture('const later = 1;', 'const later = 1;', false);
    actions.close(path);
    await actions.openFile(path);
    capture('const later = 1;', 'const later = 1;', false);
    actions.edit(path, 'const failed = 1;');
    const failed = actions.save(path);
    pending!.reject(Error('bounded rejection'));
    pending = null;
    check(!(await failed));
    capture('const failed = 1;', 'const later = 1;', true);
    return { profile: 'antimatter_native_document_v1', frames };
})();
