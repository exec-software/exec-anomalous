//! Profile-12 nested nominal records and concrete generic identity admission.
//!
//! Record fields contain either i64 or an exact declared record kind. Forward,
//! self and mutual references are allowed; resolving a field never traverses a
//! record graph. Reading an unset record field is an execution fault, while an
//! unset i64 field retains its existing zero default.
//!
//! A concrete identity may have one terminal primitive argument, for example
//! `app::box<i64>`. Only i32, i64, bool, u8, f64 and text arguments are admitted;
//! there is no instantiation, normalization, nesting, argument list or template.
//! The suffixed whole bases ptr/ref/owned/borrow/fn/tuple are reserved because
//! the reference gives them wrapper semantics; plain and qualified names retain
//! their existing nominal interpretation.
//! Generic enum tags omit that suffix and retain the full exact identity in
//! their explicit type metadata. Prior profiles retain their original grammar.

use super::{admitted_program, error, parse_profile, validation_workspace};

pub const derivation_profile: u32 = 12;

/// Validate completely before publishing any output bytes.
pub fn encode<'a>(
    edition: &'a [u8],
    output: &mut [u8],
    workspace: &mut validation_workspace<'a>,
) -> Result<usize, error> {
    super::encode_profile(edition, output, workspace, parse_profile::nested_records)
}

pub fn admit<'a, 'w>(
    image: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<admitted_program<'a, 'w>, error> {
    super::admit_profile(image, ceiling, workspace, parse_profile::nested_records)
}

/// Validate the complete identity, returning its generic-erased base spelling.
pub(super) fn identity(value: &str) -> Result<&str, error> {
    if value.len() > super::maximum_name_bytes {
        return Err(error::limit);
    }
    let Some((base, suffix)) = value.split_once('<') else {
        super::record_identifier(value)?;
        return Ok(value);
    };
    super::record_identifier(base)?;
    if ["ptr", "ref", "owned", "borrow", "fn", "tuple"]
        .iter()
        .any(|reserved| base.eq_ignore_ascii_case(reserved))
    {
        return Err(error::unsupported);
    }
    let argument = suffix.strip_suffix('>').ok_or(error::unsupported)?;
    if !matches!(argument, "i32" | "i64" | "bool" | "u8" | "f64" | "text") {
        return Err(error::unsupported);
    }
    Ok(base)
}
