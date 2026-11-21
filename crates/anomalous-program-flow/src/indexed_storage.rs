//! Profile-14 mutable integer arrays and opaque integer buffers.
//!
//! `SET_INDEX` consumes an integer array or buffer, integer index and integer
//! value, and produces no result. `CALL array__push 2` appends to an integer
//! array without a result, only after ordinary local/function call resolution.
//! Text arrays retain their previous read-only operations.
//!
//! `ALLOC i64` consumes an explicit integer count and produces an opaque buffer;
//! `FREE` consumes a buffer. `GET_INDEX` also reads integer buffers. Buffer
//! values have no signature spelling, integer representation, global slot,
//! arithmetic, cast, length operation or external authority. Locals and the
//! operand stack can retain aliases; release state is checked at execution.
//! Allocation sizes, stale aliases and shared arena capacity are runtime checks,
//! not inferred from literals during admission. All earlier profiles retain
//! their original grammar, types and authority requirements.

use super::{admitted_program, error, parse_profile, validation_workspace};

pub const derivation_profile: u32 = 14;

/// Validate completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    super::encode_profile(edition, output, workspace, parse_profile::indexed_storage)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    super::admit_profile(image, ceiling, workspace, parse_profile::indexed_storage)
}
