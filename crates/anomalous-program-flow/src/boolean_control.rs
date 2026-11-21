//! Profile-15 boolean control flow and finite f64 builtin console output.
//!
//! Branches and NOT accept integers or booleans; NOT returns an integer. Only
//! existing console-name CALL fallbacks gain finite f64 output. Raw CONSOLE and
//! intrinsic INTEROP_CALL forms retain their integer/text contract and distinct
//! formatting. Every output form still requires console authority.
//!
//! CFG predecessors may join integer and boolean locals into a private union.
//! Reading such a local requires an immediately adjacent CAST bool instruction;
//! labels break adjacency, descriptive source metadata does not. The runtime
//! cast still rejects integer values outside 0/1. Names carry no typing power.
//! Stack joins and declared signatures remain exact, and no union is exposed
//! in the validated plan. STORE kills the prior state to the exact stored kind.
//! Conflicting exact bool/integer stores are checked after the fixed point, so
//! a real loop-header union is independent of predecessor visit order; ordinary
//! sequential type changes still fail. Other local types never join this way.

use super::{admitted_program, error, parse_profile, validation_workspace};

pub const derivation_profile: u32 = 15;

/// Validate completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    super::encode_profile(edition, output, workspace, parse_profile::boolean_control)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    super::admit_profile(image, ceiling, workspace, parse_profile::boolean_control)
}
