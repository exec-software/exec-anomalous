# ADR 0040: Separate entity identity, authentication, and authority

- Status: Proposed
- Date: 2026-09-26

## Context

The user selected a native identity model without usernames or passwords and
expects future biometric interaction. Humans and agents should participate as
entities, with explicit answers for administration, delegation, and recovery.
The [vision](../../VISION.md) already requires common human/machine policy and
narrow, visible authority; it places identity policy outside the kernel.

The existing kernel has synthetic principal metadata and bounded task-local
sigil mechanisms. [ADR 0023](0023-prove-bounded-authenticated-source-session-routing.md)
accepts a host routing proof that consumes an external authentication assertion.
Neither authenticates an entity. Current native session delivery carries origin
only; the experimental invocation header cannot turn a claimed identity into
authority. End-to-end authentication remains unimplemented.

The design requires holder-bound delegation, scoped budgets, revocation, and
retained session state. Native enrollment, trust-root governance, biometric
support, and durable recovery remain unimplemented. The
[architecture note](../architecture/identity-and-authority.md) records those
independent requirements and integration boundaries.

## Proposed decision

An **entity** is a stable, opaque identity scoped to a realm: a local machine,
personal environment, or explicitly governed trust domain. It is independent
of display aliases, current keys, sessions, task identities, and privileges.
There is no universal identifier that must correlate every service or device.
Human, agent, and service kinds describe entities; they confer no authority.

Authentication establishes an exact live session using an enrolled, replaceable
authenticator. Native enrollment and routine authentication require neither a
username nor a password, and must not silently fall back to either. Optional
display names remain useful for people. Authentication alone grants no rite,
administrative privilege, or access to protected data.

Biometrics are a proposed local user-verification method for activating a
protected cryptographic key, not the entity identifier or key material. Keep
raw samples and templates out of model context, ordinary applications, and
central login databases. Support modular passwordless authenticators and
accessible non-biometric alternatives. Presence-only touch must not be reported
as verification of a particular human. NIST likewise treats biometrics as an
activation factor used with a physical authenticator, rather than a standalone
authenticator; this proposal does not select an assurance level or implement
that standard. See [SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html).

Authority remains a separate grant: exact holder/session, target, operation,
scope, limits, lifetime, and delegation/revocation lineage. Humans and agents
use the same enforcement rules; their authentication evidence can differ.
Agents receive their own sponsored identities and constrained workload sessions,
not the sponsor's identity or root key. A model never needs long-lived root
private material in its conversational context.

Administration consists of scoped maintenance and recovery rites, including
authenticator enrollment, authority delegation, service management, and trust-root
updates. Do not add a universal `is_admin` bypass. The first-owner ceremony must
bind a local realm, initial owner authenticators, root-governance policy, and
recovery choices through a trusted path. Subsequent root changes require
explicit existing authority; an untrusted request cannot choose its own issuer.

The user-space identity/policy services must bind authenticated requests to the
complete operation and body, intended service/audience, target, nonce or exact
sequence, session incarnation, and relevant policy/revocation epoch. Authority,
expiry, freshness, and effect preconditions are rechecked at commit. Presentation
of an earlier valid claim must not restore revoked or exhausted authority.
Keep trusted intent confirmation separate from merely unlocking a key. Policy
may authorize routine operations within an existing grant without asking for
approval each time.

Local operation must not depend on a mandatory cloud identity service. Persist
revocation and security accounting, or use a durable new epoch that rejects all
earlier authority after restart. Offline grants need explicit scope, freshness,
and expiry policy; an offline verifier cannot promise immediate knowledge of
remote revocation. Clock rollback and unavailable durable state must not renew
expired or revoked authority.

Recovery is a separately governed authority path. Enrollment, key rotation,
identity recovery, and decryption of old data are different operations. If all
keys and every prearranged recovery path are lost, access may be irrecoverable;
there is no hidden password or universal recovery override. Changing identity
ownership does not automatically recover encryption keys.

Keep authenticators, biometric adapters, enrollment, policy, and crypto/token
parsers in replaceable user-space services. The kernel provides isolation,
unforgeable task/channel provenance, capability checks, lifecycle/revocation
mechanisms, and time primitives. Linux UID 0, setuid bits, and equivalent foreign
administrator identities never mint native authority. Veils translate their
local semantics only inside explicitly delegated native bounds.

## Consequences and evidence required

This direction selects no cryptographic suite, credential/token encoding,
serialized entity representation, or stable native ABI. It does not import an
identity implementation, implement biometrics, establish secure attention, attest trustworthy
agent behavior, or claim prompt injection is solved.

The smallest later integration proof should bind an externally verified entity
session to one exact native task/channel, issue authority separately, and reject
copied identities, wrong audience/body, replay, revoked preparations, stale
sessions, restarted issuers, rights broadening, and exhausted generations without
partial mutation. Test human and agent paths with equal enforcement and distinct
attribution. A host assertion remains a fixture until real authentication and
trusted enrollment are demonstrated. A principal-bearing transport needs its
own versioned contract rather than changing native v1 delivery implicitly.

This increment adds documentation only. Existing kernel and execution-readiness
gates remain open; it does not claim their completion or alter desktop behavior.

## Alternatives

- **Native shared-secret accounts:** rejected by the user's passwordless
  direction; display aliases need not become usernames or credentials.
- **Entity ID alone or biometric-derived keys:** rejected because naming an
  entity cannot establish authenticated control or permission. Local biometric
  verification activates a separately protected key instead.
- **Universal administrator bypass:** rejected because maintenance and recovery
  powers can be independently scoped, delegated, revoked, and inspected.
- **Direct external token/crypto import:** deferred to independent review of
  encoding, cryptographic assumptions, trusted integration, and durable state.
