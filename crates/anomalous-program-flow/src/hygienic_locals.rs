//! Profile-13 canonical compiler-generated function-local storage identities.
//!
//! `$ny.local.<counter>.<name>` is a complete, opaque local name. The counter
//! is canonical ASCII decimal (zero or no leading zero), never an array index,
//! allocation count or inferred sequence. The final name is a simple ASCII
//! identifier; the complete spelling remains bounded by `maximum_name_bytes`.
//! Neither the counter nor the source-name suffix is removed for lookup.
//!
//! These names are accepted by LOAD_LOCAL, STORE_LOCAL and the existing local
//! indirect CALL path. Parameters, functions, labels, globals and field names
//! retain their previous grammar. Source-debug strings remain descriptive and
//! cannot create, initialize or type a local. All earlier profiles are unchanged.

use super::{admitted_program, error, parse_profile, validation_workspace};

pub const derivation_profile: u32 = 13;

/// Validate completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    super::encode_profile(edition, output, workspace, parse_profile::hygienic_locals)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    super::admit_profile(image, ceiling, workspace, parse_profile::hygienic_locals)
}
