# Bounded user mapping and physical-access planning

Date: 2026-09-26

## Scope

The user continued the kernel foundation work before the native launch contract,
Reliquary, and Linux-first compatibility. Proposed
[ADR 0041](../../docs/adr/0041-plan-bounded-user-mappings-and-physical-access.md)
isolates the next step into inert user-mapping and physical-window geometry.
The [readiness audit](../../docs/architecture/execution-readiness.md) remains the
record of broader unresolved execution prerequisites.

The current mapper owns three extents in one leaf-table branch, reserves PML4
slot zero, and accesses physical storage through identity-mapped pointers.
Planning ordinary low executable addresses cannot make them safe to activate.
The new components must leave those live restrictions and accesses unchanged.

## Selected proof obligations

The user planner accepts finite explicit extents and supervisor/guard exclusions,
validates lower-half canonical 4-KiB geometry and requested user permissions,
rejects W^X and collisions, and computes independent data and table demand.
The experimental limits are eight mappings, eight exclusions, and 32 user-tree
table frames including the root, with separately supplied data/table budgets.
Unique branch prefixes must count shared and sparse mappings correctly; future
supervisor/direct-map tables need a separate merged budget. Overlapping
exclusions represent a union and consume no mapped pages.
Executable membership must preserve gaps rather than widen to one enclosing
interval. Complete-range data checks precede bounded 2-MiB branch enumeration;
the implementation does not walk every page of an untrusted large extent.

The physical-window component describes a checked high-half virtual aperture for
a physical range. It keeps physical and virtual geometry distinct and rejects
invalid or overflowing complete subranges, including byte-sized translations.
The physical limit is the page-entry format's 52-bit envelope, not a claim about
CPUID width or available RAM. Calculating an address is not proof
of allocation, installed mapping, current access, or authority to touch memory.
No pointer, lease, table population, CR3, or launch API belongs in this proof.

The implementation is in
[`mapping_plan.rs`](../../kernel/src/arch/x86_64/mapping_plan.rs) and
[`physical_window.rs`](../../kernel/src/arch/x86_64/physical_window.rs), both
wired as private x86-64 modules. The four-region planning fixture accounts for
seven data pages and nine user-tree table frames: the root, five tables shared
by the low regions, and three for the distant stack. These are inert resource
requirements; no corresponding allocations or mappings are performed.

Neither component selects a permanent high-half layout, freezes an ABI, parses
ELF, or establishes live low-address support. The future mapper still needs a
safe privileged access path under changing roots and failure-atomic staged
ownership with complete withdrawal, clearing, and reclamation.

## Validation — passed

`cargo test -p kernel --lib arch::x86_64` passed all 70 selected tests, including
11 mapping-planner tests with a separate bounded `BTreeSet` page oracle and
eight physical-window tests. They cover range/canonical edges, exclusion
collisions, permission rejection, exact budgets, 2-MiB/1-GiB/512-GiB crossings,
sparse/shared branches, complete physical/virtual subranges, and unchanged
inputs and prior plans after rejection. The initial handwritten four-region
table expectation of ten was corrected to nine; the independent page oracle
confirmed nine. This corrected the fixture expectation, not the planning algorithm.

Freestanding
`cargo clippy -p kernel --target x86_64-unknown-none -- -D warnings` passed.
Independent code review reported no actionable finding. Focused evidence is
`focused-tests.log` and `focused-clippy.log` under
an external validation directory (not distributed).

The complete existing **23-target matrix passed**, totaling **1,089.4 seconds**
of combined target time and finalized at `2026-09-27T00:35:58+00:00`.
The full kernel suites passed **361 default** and **404 all-feature** tests.
The finalizer verified that all **230 frozen source/configuration files** remained
unchanged. All targets returned zero:

- `check`, `smoke`, `fault-smoke`, `circle-smoke`, `control-smoke`, `rite-smoke`;
- `scheduler-smoke`, `dispatcher-smoke`, `supervision-smoke`,
  `shell-vessel-smoke`, `memory-smoke`, `interrupt-smoke`, `input-wait-smoke`,
  `shell-smoke`;
- `display-vessel-smoke`, `supervised-renderer-smoke`,
  `semantic-surface-transport-smoke`, `semantic-presentation-smoke`,
  `desktop-smoke`, `workspace-smoke`, and `normal-exclusion`;
- additional `dispatcher-smoke` and `workspace-smoke` with KVM.

Both TCG and KVM dispatchers retained the exact 14-record proof: 64 preemptions,
80 timer ticks, 66 dispatches, one contained fault, 16 survivor ticks, one exit,
intact canaries, and restoration of all 65,112 initial free frames. Both workspace
runs retained 21 keyboard, 410 pointer/Appearance, and 23 graphics frame checks,
resource restoration, and shell fallback.

The final normal, dispatcher, and workspace images are byte-identical to their
pre-change counterparts:

| Image | Before and after SHA-256 |
| --- | --- |
| `anomalous.img` | `bb36c1cf49de28a594f83a5ea3b81b2ba99cd9b5c1c6997953f2dc0e45957e9f` |
| `dispatcher-test.img` | `bf7a36b68bd696c3b15898350b82421793ec72458098240540f0ead9a53f7e6c` |
| `workspace-dev.img` | `7ef45ff45e6592ee1399f8922e95a87dc946afcbdb19004464a445e2d1275e9d` |

The local validation report (`memory-planning-validation.md`; not distributed)
links every target log and timing. The
machine-readable summary (`summary.json`; not distributed)
records outcomes, predicates, and evidence hashes; the
frozen source manifest (`source-sha256.json`; not distributed)
records the validated source set. These are development-session artifacts outside
the repository. Native regression success demonstrates retained runtime behavior;
it does not demonstrate activation of a planner that no live mapper consumes.

No boot or live-mapper change creates a new VirtualBox exercise requirement in
this increment, and no new run is claimed. The preceding checkpoint's isolated
VirtualBox attempt was blocked by host KVM ownership of AMD-V (`VERR_SVM_IN_USE`);
that historical result is not validation of this change.
