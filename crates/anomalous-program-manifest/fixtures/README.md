# Manifest derivation fixtures

`arithmetic.nil` and `console.nil` are explicitly hand-authored textual NIL
fixtures for this bounded admission profile. They are not compiler output and
do not qualify a source-to-admitted-edition pipeline. The retained `.ny` files
exercise that separate compiler boundary.

`compiler-arithmetic.nil` and `compiler-console.nil` are complete, unmodified
outputs from those sources, produced with the installed SDK on 2026-11-11:

```sh
runtime/nilcompiler --quiet --no-stdlib --no-bundle -o compiler-arithmetic.nil arithmetic.ny
runtime/nilcompiler --quiet --no-stdlib --no-bundle -o compiler-console.nil console.ny
runtime/nilcompiler --inspect-context compiler-arithmetic.nil
runtime/nilcompiler --inspect-context compiler-console.nil
```

Compilation and authored-context inspection succeeded. These two outputs are
**negative admission fixtures**: the compiler also emits a `void` registration
helper containing `INTEROP_CALL nilaix register_ai_context_internal 5 void`.
That external operation is outside this profile, even though the emitted helper
is unused. It is neither stripped nor treated as authority-free metadata.
The raw outputs also retain the compiler's trailing spaces and final blank
lines. Their pinned hashes take precedence over whitespace normalization;
exclude only these two files when checking a patch for whitespace errors.

The inspected compiler source suppresses this helper in its internal release
mode, but its CLI does not expose that mode; normal compilation uses debug
mode. A supported compiler-to-admission bridge remains a separate gate.
The source revision recorded by the installed SDK includes dirty working-tree
state, so the exact tool and artifact hashes are the reproducible identity.

| Evidence | SHA-256 |
| --- | --- |
| Installed `nilcompiler` | `6764709d059797bce815b51ea73b2adfd8d6404cbcfa0bf21c8b01e9f0a76f26` |
| SDK manifest | `7292b787e330aae54f4fe172f03ea436cfba09f7dae70546545c2d4eb40370ea` |
| `arithmetic.ny` | `66755488485dd52a2bea34e185fdd5930e505e58f464a56292e63290e9563bc2` |
| `console.ny` | `d8f42bb147a4c16e2a35070f3adffe4f76d3882ea736ae66112813ddaec79245` |
| `compiler-arithmetic.nil` (1286 bytes) | `ba2e98aa1979bfd6ddecb620a320f3b7dc0097801f2b46f1fd9cae9f0409f79f` |
| `compiler-console.nil` (1012 bytes) | `6df7742ffb25c7f89d3ac6ce432cc0c11eb8aac6f76d5252858cbd15c160e876` |

SDK source metadata: revision `241eb762e106079f3ac362f879c7a86d737948af`,
`worktree_dirty: true`. Inspection of the hand-authored positive fixtures and
the complete host qualification are recorded in the
[manifest admission log](../../../notebook/Development/2026-11-11%20Program%20Manifest%20Admission%20Log.md).
