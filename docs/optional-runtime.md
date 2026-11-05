# Optional language runtime and reference inputs

The public home for Ny and Matrix is the
[NIL framework](https://github.com/exec-software/exec-nil-framework).
Anomalous's optional language experiments currently retain a historical SDK
contract with `nycompiler`, `nyinterpreter`, `nyassembler`, and `nylinker` plus
`nystd/` and `nylib/`. The current public NIL SDK uses different command names
and layout. Compatibility with it is not qualified here; copying or renaming
binaries is not sufficient evidence of compatibility.

The current Rust kernel, Threshold boot loader, and Python Eidolon reference
remain available independently. Ny is the official future language direction;
privileged migration still requires the accepted contracts and validation gates.
Oculus and Archon are scaffolds. They do not replace the Rust desktop or provide
production security scanning.

## Historical SDK route

If you maintain a compatible source checkout implementing the documented legacy
SDK builder contract, select it explicitly:

```sh
NY_REPO_DIR=/path/to/compatible-sdk-source ./scripts/refresh-ny-runtime.sh
```

The script invokes that trusted checkout's `scripts/build-sdk.sh` and validator.
It defaults to clean source and running the toolchain tests. An explicit
`NY_ALLOW_DIRTY_SOURCE=ON` requests a development SDK and must be reported as such.
Generated `runtime/` and `.runtime/` directories are ignored and never published.
No sibling checkout is selected automatically. Until a public-NIL adapter is
implemented and qualified, the historical route is optional research tooling,
not a complete public installation path.

## External application comparisons

`scripts/native-application-reference.cjs` and
`scripts/native-document-reference.cjs` compare bounded contracts against a
separately installed [Antimatter checkout](https://github.com/exec-software/exec-antimatter).
Pass its path explicitly as their first argument. They load that checkout's
build dependencies and selected source; run them only against a trusted source
version. They are optional comparison harnesses, not part of Threshold boot.
Recorded historical comparisons do not guarantee that the latest Antimatter
source still satisfies an earlier reference contract.

Linux-loader proofs likewise need the exact externally provisioned files and
hashes recorded in their proof configuration. No host executable, system
library, SDK, model weight, or generated guest image is included in this source
release. See the development guide and each runner's validation contract.
