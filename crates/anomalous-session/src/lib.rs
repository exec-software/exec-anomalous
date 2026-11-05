#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Allocation-free model of one private Anomalous shell-session ingress.
//!
//! This crate is a host-tested state-machine proof. It does not parse or execute
//! source, authenticate an origin, grant authority, expose physical input, or
//! define a kernel or wire ABI.
//!
//! Every content-derived getter is a local disclosure boundary. This includes
//! action `kind`, `expose_text`, `expose_erase_count`, `expose_str`, and
//! `expose_byte_len`: observing a discriminator, edit count, exact length, or
//! text is not permission to log, persist, forward, or format it.
//!
//! The included compile-fail matrix independently freezes implicit ownership,
//! formatting, and hashing traits for every exported owner, preparation,
//! receipt, and delivery type. Copyable scalar identities and result enums are
//! deliberately excluded; replay protection comes from the session state machine.
#![doc = include_str!("privacy_compile_fail.md")]

mod action;
mod identity;
mod observation;
#[path = "private_text.rs"]
mod private_text_storage;
mod session;

pub use action::{
    action_error, erase_scalar_limit, semantic_action, semantic_action_kind, submission_origin,
};
pub use identity::{
    consumer_identity, identity_error, language_version, route_epoch, session_identity,
    session_revision, submission_sequence, wait_identity,
};
pub use observation::{observation_event, observation_result, privacy_observation};
pub use private_text_storage::{private_text, private_text_capacity, private_text_error};
pub use session::{
    close_reason, close_receipt, prepare_rejection, prepared_submission, prepared_wait,
    publish_receipt, publish_rejection, receive_preparation, session_error, session_submission,
    shell_session, wait_cancellation, wait_commit, wake_acknowledgement, wake_outcome,
};

#[cfg(test)]
extern crate std;
