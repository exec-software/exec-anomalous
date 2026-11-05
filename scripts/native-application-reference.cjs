// Run against an existing Antimatter checkout; installs nothing and writes no bundle.
// Usage: node scripts/native-application-reference.cjs /path/to/exec-antimatter
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
assert.ok(process.argv[2], 'Pass a trusted Antimatter checkout path explicitly.');
const antimatter = path.resolve(process.argv[2]);
const source = path.join(__dirname, 'native-application-reference.ts');
const result = require(path.join(antimatter, 'node_modules/esbuild')).buildSync({
    stdin: { contents: fs.readFileSync(source, 'utf8'), resolveDir: antimatter,
        sourcefile: 'native-application-reference.ts', loader: 'ts' },
    bundle: true, platform: 'node', format: 'cjs', write: false, metafile: true,
    logLevel: 'warning', absWorkingDir: antimatter,
});
let observed;
vm.runInNewContext(result.outputFiles[0].text, {
    console: { log(value) { assert.equal(observed, undefined); observed = JSON.parse(value); } },
}, { timeout: 5000 });
const expected = JSON.parse(fs.readFileSync(path.join(__dirname,
    '../tools/xtask/testdata/antimatter-editor-reference.json'), 'utf8'));
assert.deepEqual(JSON.parse(JSON.stringify(observed)), expected);
const sha256 = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const inputs = Object.keys(result.metafile.inputs).filter(name => !name.startsWith('<')
    && name !== 'native-application-reference.ts').sort();
const source_hashes = Object.fromEntries(inputs.map(name => [name, sha256(path.resolve(antimatter, name))]));
source_hashes['reference_harness'] = sha256(source);
console.log(JSON.stringify({ matched: true, reference: observed, source_hashes }, null, 2));
