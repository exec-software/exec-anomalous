# Complete metadata-only compiler fixtures

These three `.nil` files are complete, unmodified outputs of the isolated
`codex/context-metadata-only` compiler branch. `provenance.json` records the
source commit, binary SHA-256, exact commands and output identities. The build
uses LLVM disabled; this qualifies textual NIL generation, not LLVM native or
JIT routes. The installed Anomalous SDK is not replaced by this checkout.

The explicit `--no-context-registration` option preserves authored `@ai_context`
annotations while preventing generation of the external `nilaix` registration
helper. Default compiler behavior remains unchanged. The parent directory's
complete old compiler outputs remain refusal fixtures; no helper is stripped
from any retained edition. Preserve the generated whitespace and final blank
lines exactly.

Arithmetic and console exercise zero and one derived console service. The
larger source deliberately generates an 8,261-byte edition, requiring three
bounded reads after packaging. Native proof tooling builds canonical packages
and deliberate corruptions from these snapshots, checks independent length,
FNV and SHA-256 pins, and retains them under `build/native-manifest/`. Each
negative package has a correct independent transport checksum so that its
semantic refusal is exercised inside userspace.

This fixture set establishes bounded compiler-to-admission coverage. It does
not establish execution of these editions or general NIL/ELF compatibility.
