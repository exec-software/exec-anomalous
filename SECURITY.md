# Security policy

Anomalous is pre-alpha operating-system research. It is not hardened, is not
suitable for production or sensitive workloads, and currently provides no
security support guarantee or response-time SLA.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use the
repository's
[private vulnerability reporting](https://github.com/exec-software/exec-anomalous/security/advisories/new)
flow. If that facility is unavailable, contact the project owner privately
through the verified contact information on the GitHub organization.

Include the affected revision, environment, reproduction steps, impact, and any
known mitigation. Please allow coordinated disclosure while the report is
triaged.

## Areas of interest

Reports are especially useful around:

- Threshold boot-chain integrity and the boot_omen contract;
- memory safety, page tables, interrupts, and privilege transitions;
- sigil validation, delegation, revocation, and confused-deputy behavior;
- IPC parsers and vessel isolation;
- Reliquary parsing, signatures, provenance, and Covenant transactions;
- driver familiars and device DMA;
- compatibility veils and foreign binary parsing;
- exec-hag authorization and untrusted context handling; and
- recovery paths that cross a trust boundary.

## Security posture

The Rust kernel runs at ring 0. Feature-gated fixtures exercise bounded ring-3
isolation, fault containment, and owned address spaces; these proofs are not a
general application sandbox or a production process model. Authentication,
persistent trust, drivers, and recovery still have open boundaries. Structured
events and type-safe source do not make the system secure.
The threat model evolves in [docs/threat-model.md](docs/threat-model.md).
