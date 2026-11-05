# Anomalous nomenclature

The occult vocabulary is a technical namespace. Each term has one canonical
meaning so code remains searchable and documentation remains precise. On first
use in external-facing prose, include the familiar computing term in
parentheses.

| Anomalous term | Engineering meaning |
| --- | --- |
| **Threshold** | The Anomalous-owned boot loader and its firmware frontends |
| **boot_omen** | Versioned data handed from Threshold to the kernel |
| **vessel** | Supervised, restartable user-space service or daemon |
| **familiar** | A vessel that mediates a device or driver protocol |
| **circle** | Isolated protection domain containing memory and authority |
| **rite** | Discoverable, versioned operation schema |
| **invocation** | A request to perform a rite, carrying identity and trace context |
| **sigil** | Unforgeable, delegable reference to specific authority |
| **oracle** | Observability vessel for structured events, traces, and crash artifacts |
| **Reliquary** | Native signed executable and package artifact format |
| **covenant_manifest** | Declarative document describing desired environment state |
| **Covenant** | Package and environment transaction system |
| **veil** | Isolated foreign-platform compatibility subsystem |
| **Canticle** *(proposed)* | Typed, capability-explicit Anomalous command language and its machine-equivalent AST |
| **Eidolon** *(proposed)* | Lossless framebuffer-trace codec and container |

Names should be evocative without becoming vague. For example, a device
familiar is still documented as a driver vessel, and metrics emitted by the
oracle still use conventional units and schemas.

New terms require an ADR when they introduce a platform-level abstraction.
Words used only as decorative branding do not belong in code.
