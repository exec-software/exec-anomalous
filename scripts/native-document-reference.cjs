// Usage: node scripts/native-document-reference.cjs /path/to/exec-antimatter
// Uses the existing checkout and dependencies; installs nothing, writes no file.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
assert.ok(process.argv[2], 'Pass a trusted Antimatter checkout path explicitly.');
const antimatter = path.resolve(process.argv[2]);
const source = path.join(__dirname, 'native-document-reference.ts');
const result = require(path.join(antimatter, 'node_modules/esbuild')).buildSync({
    stdin: { contents: fs.readFileSync(source, 'utf8'), resolveDir: antimatter,
        sourcefile: 'native-document-reference.ts', loader: 'ts' },
    bundle: true, platform: 'node', format: 'cjs', write: false, metafile: true,
    define: { 'process.env.NODE_ENV': '"production"' },
    logLevel: 'warning', absWorkingDir: antimatter,
});
const context = {};
vm.runInNewContext(result.outputFiles[0].text, context, { timeout: 5000 });
const sha256 = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const inputs = Object.keys(result.metafile.inputs).filter(name => !name.startsWith('<')
    && name !== 'native-document-reference.ts').sort();
const source_hashes = Object.fromEntries(inputs.map(name => [name, sha256(path.resolve(antimatter, name))]));
source_hashes.reference_harness = sha256(source);
const deadline = setTimeout(() => {
    console.error('Antimatter document reference did not complete');
    process.exit(1);
}, 5000);
context.reference.then(reference => {
    clearTimeout(deadline);
    console.log(JSON.stringify({ matched: true, reference, source_hashes }, null, 2));
}).catch(error => { clearTimeout(deadline); console.error(error); process.exitCode = 1; });
