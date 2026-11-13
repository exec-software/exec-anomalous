# Anomalous

Anomalous is an experimental operating system for people and software agents.
It currently combines a Rust kernel, its own x86-64 boot loader, capability-oriented
execution research, and an accessible desktop whose human and machine views
share explicit state and policy.

**Status: pre-alpha research.** This is a source release for contributors and
researchers. It is not a production operating system, an installable daily-use
desktop, or a security boundary for sensitive workloads.

## What is implemented

- **Threshold** boots a BIOS disk image into a separate Rust x86-64 kernel.
- Bounded proofs exercise physical-memory ownership, page-table construction,
  ring-3 execution, interrupt delivery, preemption, fault containment, and
  supervised replacement.
- A development workspace provides software-composited windows, mouse and
  keyboard interaction, two workspaces, Light/Dark themes, an Appearance panel,
  and a Graphics Lab. These are built-in demonstrations in one workspace
  vessel, not general application processes.
- Native application, document, editor, and runtime fixtures explore bounded
  loading, service ownership, persistence, and workspace integration.
- Linux ELF planning and interpreter/service-boundary proofs exercise an
  unchanged Linux interpreter with constrained userspace policy. They do not
  yet run general Linux applications or Electron.
- An experimental host-only NIL executor runs admitted integer/text programs
  with bounded console output. A separate profile-2 increment adds validated
  static calls, branches, loops and bounded recursion, qualified by 25 final
  checks and a 77-case bounded proof.
  [ADR 0105](docs/adr/0105-validate-bounded-function-control-flow-before-host-execution.md)
  keeps full corpus qualification and native execution as separate gates.
  [Profile 3](docs/adr/0106-admit-bounded-compiler-scalar-control-flow.md) adds
  bounded compiler temporary locals and scalar range-loop forms, qualified by
  27 final checks and a 61-case bounded proof.
  [Host conformance](docs/adr/0107-separate-host-conformance-from-production-admission.md)
  separately validates and executes raw scalar editions without manufacturing
  authored intent; production admission retains its required intent and authority
  checks. [Profile 4](docs/adr/0108-add-bounded-computed-text-and-hexadecimal-integers.md)
  adds bounded computed text and signed hexadecimal integers.
  [Profile 5](docs/adr/0109-add-bounded-immutable-typed-arrays.md) adds immutable
  typed arrays, lengths and indexed reads.
  [Profile 6](docs/adr/0110-add-bounded-internal-callable-values.md) adds typed
  internal function references and local indirect calls.
  [Profile 7](docs/adr/0111-add-bounded-nominal-integer-records.md) adds nominal
  records with shared mutable integer fields and typed helper arguments/results.
  [Profile 8](docs/adr/0112-add-bounded-module-globals-and-integer-text.md) adds
  per-execution integer globals and bounded integer-to-text conversion.
  Thirteen of 45 unchanged raw corpus invocations match; full corpus qualification
  and native execution remain gated.
- Host-side libraries explore typed session routing, shared semantic scenes,
  presentation acknowledgement, and recovery.
- **Eidolon** supplies an independent Python reference codec for lossless
  framebuffer traces and an optional experimental Ny implementation.
- **Oculus** (Matrix desktop) and **Archon** (Matrix security tooling) are early
  language/runtime scaffolds, not replacements for the existing Rust proofs.

General application support, a production filesystem, authentication,
networking, package management, and broad foreign compatibility remain research
goals. Ny is the official language direction; verified Rust and assembly remain
authoritative until separately qualified replacements meet the migration gates. The [current-state notebook](notebook/Overview/Current%20State.md)
and [execution-readiness gates](docs/architecture/execution-readiness.md)
distinguish implemented mechanisms from proposals. Dated test results describe
the recorded checkpoint; they are not a claim that this public export was
rebuilt on every supported host.

## Build and boot

The current bring-up host is x86-64 Linux. Install rustup, GNU Make, Bash, GNU
binutils, and QEMU (`qemu-system-x86_64`). The pinned Rust toolchain and required
components are listed in [rust-toolchain.toml](rust-toolchain.toml). VirtualBox
7 is an optional interactive route.

```sh
git clone https://github.com/exec-software/exec-anomalous.git
cd exec-anomalous
make image
make smoke
make workspace-run
```

`make image` creates `build/anomalous.img`; `make smoke` runs a bounded headless
boot check. `make workspace-run` builds a separate development image and opens
QEMU. It uses KVM when available and falls back to software emulation; set
`ANOMALOUS_QEMU_ACCEL=tcg` to select software emulation explicitly. Close QEMU
to stop the guest. Do not write development images to a physical disk.

The basic Rust boot path does not import the optional Ny SDK or an AI model.
Language experiments and external-binary proofs need separately provisioned
inputs; see [optional runtime requirements](docs/optional-runtime.md).
Build outputs under `build/` and `target/` are ignored. For a shared workspace, use a disposable checkout and keep generated
artifacts outside the source tree you intend to publish.

See the [development guide](docs/development-guide.md) for the complete target
matrix, desktop controls, VM workflow, and checkpoint history. Hardware-facing
changes need the relevant native proof in addition to unit tests. Full validation
with `make check` also constructs strict Linux-loader fixtures and requires the
exact separately provisioned system binaries recorded by those runners. It is
not a prerequisite for trying the basic boot image on an arbitrary Linux host.
The full foundations workflow is manual until its qualified host fixtures are
provisioned. Automatic CI runs the source-security audit and portable tooling
checks; these do not claim guest boot, full-matrix, or SDK qualification.

## Architecture and documentation

| Topic | Start here |
| --- | --- |
| Design direction | [Vision](VISION.md), [architecture overview](docs/architecture/overview.md) |
| Kernel and execution | [Kernel](docs/architecture/kernel.md), [execution readiness](docs/architecture/execution-readiness.md) |
| Security boundaries | [Threat model](docs/threat-model.md), [security policy](SECURITY.md) |
| Accepted and proposed decisions | [Architecture decisions](docs/adr/README.md) |
| Technical research and measurements | [Notebook guide](notebook/README.md), [notebook index](notebook/Index.md) |
| Terms | [Nomenclature](docs/nomenclature.md) |
| Language experiments | [Optional runtime](docs/optional-runtime.md), [Eidolon](eidolon/README.md), [Oculus](oculus/README.md), [Archon](archon/README.md) |

Project terms have defined technical meanings: a **vessel** is an isolated
service or execution owner, a **sigil** represents scoped authority, and a
**rite** is a typed operation. Planned **Reliquary** artifacts, **Covenant**
package transactions, and general compatibility **veils** remain under design;
the bounded loader and userspace-policy fixtures do not establish broad compatibility. Human and agent clients must respect the same authorization
boundary.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md). Preserve narrow
unsafe and assembly boundaries, document invariants, and include evidence for
hardware or security claims. Report vulnerabilities privately using
[SECURITY.md](SECURITY.md).

Created by **Nathan Young**. Execute Software and community contributors are
recognized in [CONTRIBUTORS.md](CONTRIBUTORS.md); [AUTHORS.md](AUTHORS.md) records
project attribution.

The repository uses Execute rolling versions (`v1`, `v2`, and so on) in
[VERSION](VERSION). Cargo component versions describe API compatibility and are
independent of the repository release number.

## Support and enterprise inquiries

Support development through [Ko-fi](https://ko-fi.com/execsoftware).
For enterprise licensing and support, contact
[admin@executesoftware.com](mailto:admin@executesoftware.com).

Enterprise agreements may cover support, certification, warranties, indemnity,
managed distribution, and lifecycle commitments. Availability and terms require
an explicit agreement; donations do not purchase those services or change the
open-source license.

## License

Anomalous is available under the [MIT License](LICENSE), copyright 2026 Nathan
Young. Separate enterprise agreements do not reduce the rights granted by MIT.
