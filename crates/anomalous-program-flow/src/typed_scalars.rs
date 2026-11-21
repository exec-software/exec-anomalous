//! Profile-11 typed scalar admission. Booleans and bytes require explicit
//! casts from integer literals; binary64 literals retain their exact bits.
//!
//! Decimal floating literals require a point or exponent marker, an optional
//! sign, at least one mantissa digit and digits after any exponent marker.
//! Hexadecimal spellings remain integer-only. Signed zero retains its sign
//! for every well-formed exponent. Finite subnormals are accepted; nonzero
//! underflow to zero and overflow to infinity are syntax errors. Malformed
//! floating spellings are unsupported. Existing integer spelling is unchanged.
//!
//! `CAST bool` requires an integral value of zero or one at execution; `CAST u8`
//! retains its low eight bits. `CAST i32` and `CAST i64` explicitly bridge the
//! integral kinds. `CAST f64` is bit-preserving identity only. New kinds never
//! implicitly enter integer arithmetic, branches, aggregates or console output.

use super::{admitted_program, error, operation, parse_profile, validation_workspace};

pub const derivation_profile: u32 = 11;

/// Validate completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    super::encode_profile(edition, output, workspace, parse_profile::typed_scalars)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    super::admit_profile(image, ceiling, workspace, parse_profile::typed_scalars)
}

pub(super) fn literal(value: &str) -> Result<operation, error> {
    let unsigned = value
        .strip_prefix('-')
        .or_else(|| value.strip_prefix('+'))
        .unwrap_or(value);
    // Hex integer digits can contain e/E; decide their radix first. The new
    // profile does not widen existing signed decimal/hex integer spellings.
    if unsigned.starts_with("0x")
        || unsigned.starts_with("0X")
        || !value.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E'))
    {
        return super::integer_literal(value, parse_profile::typed_scalars).map(operation::integer);
    }

    // Match the pinned numeric_literal.hpp decimal grammar before converting.
    // This excludes locale separators, whitespace, suffixes, hexadecimal
    // floats, infinities and NaNs independently of Rust's parsing vocabulary.
    let bytes = unsigned.as_bytes();
    let mut cursor = 0;
    let mut any_digit = false;
    let mut nonzero = false;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        any_digit = true;
        nonzero |= bytes[cursor] != b'0';
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            any_digit = true;
            nonzero |= bytes[cursor] != b'0';
            cursor += 1;
        }
    }
    if !any_digit {
        return Err(error::unsupported);
    }
    if bytes
        .get(cursor)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        cursor += 1;
        if bytes
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            cursor += 1;
        }
        let exponent_start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if exponent_start == cursor {
            return Err(error::unsupported);
        }
    }
    if cursor != bytes.len() {
        return Err(error::unsupported);
    }
    // Zero is exactly representable for every syntactically valid exponent,
    // including exponents too large for a host integer. Preserve negative zero.
    if !nonzero {
        return Ok(operation::float(if value.starts_with('-') {
            1u64 << 63
        } else {
            0
        }));
    }
    let parsed = value.parse::<f64>().map_err(|_| error::syntax)?;
    let bits = parsed.to_bits();
    if bits & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000 || bits & 0x7fff_ffff_ffff_ffff == 0 {
        // Nonzero decimal underflow to zero and overflow to infinity are
        // refused; finite representable subnormals remain valid values.
        return Err(error::syntax);
    }
    Ok(operation::float(bits))
}
