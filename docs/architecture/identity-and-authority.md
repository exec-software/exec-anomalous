# Entity identity and authority

- Status: Proposed; end-to-end authentication is unimplemented
- Date: 2026-09-26
- Decision: [ADR 0040](../adr/0040-separate-entity-identity-authentication-and-authority.md)

Anomalous should let a person unlock their environment without a username or
password, and let an agent operate under its own constrained identity. Identity,
authentication, and permission remain separate. This is a target architecture,
not a description of a working login service.

## The boundaries

| Concept | Meaning | What it does not establish |
| --- | --- | --- |
| Entity | Stable opaque identity inside an explicitly governed realm. | A secret, key, executable, human-readable name, or privilege. |
| Authenticator | Replaceable enrolled means of proving control for an entity. | Authority to perform arbitrary work. |
| Session | Exact live authentication/admission incarnation with freshness and revocation state. | Permanent identity or a reusable global bearer credential. |
| Execution subject | Kernel-created live task/circle/channel incarnation. | Proof of human identity or trustworthy program behavior. |
| Sigil/grant | Specific holder, target, operations, bounds, lifetime, and delegation ancestry. | An ambient administrator role or rights inferred from a name. |

Aliases may label entities for humans, but never authenticate them. Identity
survives authorized key rotation. Correlation across realms requires an explicit
policy boundary rather than a global identifier in every request, log, or model
interaction. Sessions remain necessary security state even when there is no
traditional account-name/password login screen.

## Passwordless human and agent access

Future fingerprint or face interaction should verify a user locally and activate
a protected key. Do not derive encryption/signing keys from biometrics, use a
biometric as the entity ID, or create a central biometric login database. Raw
samples and templates stay inside the chosen protected verification boundary;
the proposed native interface exposes the verification result and bound proof,
not biometric data to AI context or ordinary applications.

This follows the separation described by
[NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html) and
[WebAuthn's biometric privacy guidance](https://www.w3.org/TR/webauthn-3/#sctn-biometric-privacy).
Those references inform the target design; Anomalous does not implement WebAuthn
or claim a certified authenticator. Actual sensor, platform, key-store, and
trusted-UI guarantees need separate evidence.

Authenticators remain modular, including accessible non-biometric passwordless
options and secondary/offline recovery devices. A device that detects touch
alone establishes presence, not necessarily a particular user's identity. Native
policy must state what evidence a selected operation needs and must not silently
substitute a password or a weaker verification method.

An agent has its own entity and short-lived workload/session binding. Sponsorship
records who admitted it and delegated which authority. Subagents receive narrower
or equal subsets with preserved ancestry; they do not impersonate the sponsor.
Keys and external credentials remain in protected authenticators or mediation
services, outside model context. A program hash, signature, or valid key proves
neither benevolent behavior nor resistance to hostile instructions.

## Administration and ownership

Administration is needed because somebody must install trusted components,
admit authenticators, grant/revoke access, maintain devices, and recover ownership.
These are separate rites and grants. An entity may hold several, but neither its
kind nor an `admin` flag bypasses authorization. A delegated service-maintenance
grant need not permit reading private files or enrolling a replacement owner.

Initial setup needs a trusted first-owner ceremony before an untrusted process
can nominate itself. Bind the realm, owner authenticators, initial trust roots,
governance of later root/policy changes, and selected recovery authorities. This
proposal leaves physical presence, trusted display/input, and root storage to
an explicit implementation proof. Root rotation, ownership transfer, and
revocation must be attributable authorized transitions, not client-provided keys
accepted as new trust roots. Governance may use independently held recovery or
multiple-party authority; no particular threshold scheme is selected here.

For a high-impact rite, a trusted surface must bind any requested confirmation
to the exact action, target, parameters, destination, scope, and consequences.
Unlocking a key or touching a sensor does not by itself approve whatever request
an application later supplies. Existing grants can cover routine operations;
the design does not impose an approval prompt on every action.

## Requests, revocation, and local availability

Authentication adapters feed an exact entity/session binding to the native
admission service. Requests bind their complete body or an unambiguous digest,
operation/schema, target, intended service/audience, nonce or exact sequence,
session incarnation, and applicable policy/revocation epoch. This specifies the
meaning to protect, not a wire format or a new hand-built cryptographic scheme.
The consumer resolves actual authority independently of caller-provided fields.

At effect commit, recheck grant liveness, issuer/session/task incarnation,
audience, body/effect binding, expiry, current policy, target preconditions, and
replay/idempotency state. Attenuation cannot broaden authority. Record revocation
before acknowledging it; define the fate of already-admitted or queued effects
explicitly. Revocation cannot undo an effect that already completed.

Local identities and permitted local work should remain available without a
mandatory cloud dependency. Retained revocations, consumed budgets, replay state,
and recovery decisions cannot disappear on service restart. Use durable state
recovery or a durably advanced epoch that invalidates earlier grants; an empty
in-memory ledger must not make an old token fresh. Trusted time and monotonic
state must prevent clock rollback from extending authority. If required state
cannot be recovered, affected operations fail closed rather than silently reset.

Offline authority has explicit lifetime, scope, and permitted revocation
staleness. A disconnected verifier cannot observe every new remote revocation;
policy may therefore require online freshness for selected operations. Document
that availability tradeoff instead of claiming instantaneous global revocation.

Lost-device recovery should use previously enrolled alternatives or deliberately
established recovery authority. Identity recovery and encryption recovery are
separate: replacing an authenticator does not decrypt old data unless the
encryption-key lifecycle included a suitable backup, escrow, or recovery path.
Losing every relevant key without such a path can mean permanent loss. There is
no hidden password fallback or universal service that can recreate a lost key.

## Mechanism and compatibility boundaries

User-space identity, authenticator, policy, approval, and recovery services own
these decisions. Kernel mechanisms enforce task/circle/channel provenance,
isolation, native capability checks, protected communication, lifecycle, and
revocation hooks. Tokens, names, biometric processing, foreign identity policy,
and provider-specific cryptography do not become kernel parsers.

Linux UID 0 and setuid, or later Darwin/other administrator conventions, describe
authority only inside their compatibility environment. They cannot create native
sigils or cross the veil's grant ceiling. The common native boundary remains
independent of a Linux account database or macOS identity provider.

## What the current code proves

| Current component | Evidence and limit |
| --- | --- |
| [Kernel identities](../../kernel/src/identity.rs) and [tasks](../../kernel/src/task.rs) | Generational execution identities with explicitly synthetic principal/origin fixtures. No entity enrollment or authentication. |
| [Kernel sigils](../../kernel/src/sigil.rs) | Task-local endpoint bindings, attenuation, transfer, coarse lineage revocation, and withdrawal. Root issuance is kernel-bootstrap-only in the bounded proof. |
| [Rite admission](../../kernel/src/rite.rs) | Exact task-bound channel grants; caller rights can narrow, never expand them. No general entity or sigil-policy integration. |
| [Session router](../../crates/anomalous-session-router/src/lib.rs) | Host-only private ownership, exact routing, revocation, and stale-preparation checks after an external assertion. No credential verification or native principal-bearing transport. |
| [Experimental ABI](../../crates/anomalous-abi/src/lib.rs) | An invocation header with claimed identity fields. Neither those fields nor native origin labels prove authenticated identity. |

The next bounded auth integration proof should join an externally verified
entity session to an exact native task/channel and separately installed grants.
Test copied identifiers, altered body/audience, replay, revoke-after-prepare,
issuer restart, task replacement, rights expansion, and exhausted generations.
Publish human/agent equivalent checks with distinct attribution. Keep fixture
assertions explicit until real enrollment and authentication replace them.

## Delegation and integration requirements

Authority delegation must bind the holder, permit only attenuation, support
descendant revocation, share explicit budgets, and retain session state that
rejects older, less restricted claims. Accountable sponsorship is separate from
the acting entity; human, agent, and service identities use the same rules.

A future implementation must establish durable ledger recovery, enrollment,
biometric adapters, and key recovery independently. Time, issuer keys, and
approval receipts require trusted integration. Each request must bind its body
digest and service audience. Token formats and cryptography need separate
review. Signed claims and context labels alone do not establish complete
mediation, safe behavior, or resistance to prompt injection. Keep logs minimally
identifying, access-controlled, and subject to explicit retention policy.
