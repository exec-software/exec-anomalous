#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Allocation-free host proof for source-bound semantic session routing.
//!
//! This crate owns two [`anomalous_session::shell_session`] routes, A and B. A
//! trusted host may bind one human-focus source and one machine-explicit source,
//! then route both through the same private semantic publication mechanism. A
//! live human binding is required before control creates the initial A focus;
//! the only transfer is A to B. Machine admission has no route selector and is
//! always bound to the current exact B incarnation, independent of human focus.
//! Repeated or reverse focus transfer is rejected.
//!
//! Control may revoke either fixed source slot by kind without possessing its
//! source token. Source revocation does not close a route or retract accepted
//! queue entries. Once human focus reaches shared route B, focus revocation is
//! rejected while the machine binding remains live; the host must explicitly
//! revoke that binding before retiring B through focus revocation.
//!
//! `host_verified_source` is deliberately named for the assertion this crate
//! consumes. This crate does not verify a password, device, person, remote peer,
//! task, circle, channel, or cryptographic credential. Human origin means only
//! that the trusted host installed that source class; it proves no presence,
//! intent, consent, or secure attention. Scalar identities and origin labels
//! confer no authority.
//!
//! Every successful constructor allocates a private, nonzero, nonwrapping,
//! process-local instance brand. Control, source, focus, and prepared owners bind
//! that unexposed brand as well as the public router identity, so equal public
//! identities across two routers confer no authority. The brand is neither
//! serialized nor stable across process restart and is not cryptographic.
//!
//! Construction rejects session-slot or consumer-value lineage aliasing between
//! A and B inside one router. The trusted host must reserve both complete
//! lineages across every concurrently live router; this crate has no global
//! route registry.
//!
//! Principal and source attribution in this proof is router-local. ADR 0022's
//! version-1 native delivery carries origin but no principal or source identity,
//! so this crate makes no end-to-end attribution claim. It defines no kernel or
//! wire ABI and never accepts physical input records.
#![doc = include_str!("privacy_compile_fail.md")]

mod identity;
mod router;

pub use identity::{
    focus_identity, identity_error, principal_identity, request_sequence, router_identity,
    source_identity,
};
pub use router::{
    focus_authority, focus_change_rejection, focus_revocation, focus_transfer,
    host_verified_source, prepared_routed_submission, route_prepare_rejection,
    route_publish_rejection, route_target, routed_publish_receipt, router_control, router_error,
    session_route_descriptor, session_router, source_kind,
};

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod router_tests;
