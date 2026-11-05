# Passwordless entity identity direction

Date: 2026-09-26

## Request and selected direction

This proposal separates stable entities for humans and agents from replaceable
authenticators and scoped authority. Native enrollment and routine use should
avoid usernames and passwords; future biometric verification should activate
protected keys locally rather than becoming identity or key material.

Proposed [ADR 0040](../../docs/adr/0040-separate-entity-identity-authentication-and-authority.md)
and the [identity/authority architecture](../../docs/architecture/identity-and-authority.md)
record the direction: stable realm-scoped entities, replaceable authenticators,
exact live sessions/tasks, and separately scoped grants. Biometrics locally
activate protected keys; they are not identity values or sources of key material.
Accessible non-biometric passwordless alternatives remain necessary. There is
no silent password fallback, central biometric login database, or model-visible
root key/template.

Administration remains a set of explicit maintenance, enrollment, trust-root,
and recovery operations rather than an entity type with universal authority.
Initial ownership needs a trusted ceremony. Routine work can proceed within
an existing grant; sensitive confirmation binds the actual intended operation.
Offline/local operation, durable revocation, restart epochs, agent sponsorship,
and request-body/audience binding are part of the proposed boundary.

## Source observations

Anomalous's existing vision already separates identity from sigils and puts
identity/policy in user space. The current kernel uses fixture principals. The
session router consumes an external verification assertion, and native input
delivery carries origin rather than an authenticated entity. No end-to-end
native authentication or biometric implementation was found.

The proposal requires holder-bound delegation, revocation, shared budgets,
and retained session state. Durable state, credential mediation, native
enrollment/recovery, and explicit request-body/audience binding still need
separate implementation evidence. This increment imports no identity library,
cryptographic primitive, token encoding, or external security claim.

Identity recovery cannot automatically recover encryption keys. If every
relevant key and all prearranged recovery paths are lost, access may be lost.
Foreign UID 0, setuid, and later macOS administration must not create native
authority. These constraints preserve the modular execution plan.

## Validation and remaining work

This is a documentation-only proposal. Repository-relative documentation links were checked.
No compiled source, boot path, ABI layout, or runtime grant behavior changes;
no build, native smoke, or authentication test pass is claimed for this increment.

Later evidence must establish real enrollment/authentication and then exact
entity-session-to-task/channel binding, with revocation and commit rechecks.
Fixtures must not be renamed into production authentication. A principal-bearing
transport and credential integration need independently reviewed contracts.
The [execution-readiness gates](../../docs/architecture/execution-readiness.md)
remain open; this direction does not complete address separation, a production
dispatcher, the Reliquary format, or Linux execution.
