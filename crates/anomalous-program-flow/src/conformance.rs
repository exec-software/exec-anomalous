//! Opt-in bounded language-conformance validation of exact raw edition bytes.
//!
//! Missing authored-main intent is permitted only here. Any annotation present
//! still follows the production grammar, placement, uniqueness and size checks.
//! Whole-artifact syntax, references, CFG, authority and resource checks remain
//! mandatory. This API neither accepts nor emits a production envelope.

use super::{
    authority, error, parse_profile, program, sha256, validation_purpose, validation_workspace,
};

/// Evidence of bounded raw language validation for a host experiment.
///
/// This is not an `admitted_program`, and it has no conversion to one. It borrows
/// the exact edition and immutable checked plan for its whole lifetime.
///
/// The fields cannot be supplied by a caller:
///
/// ```compile_fail
/// use anomalous_program_flow::conformance::checked_edition;
/// let unchecked = checked_edition {
///     edition: b"unvalidated", identity: [0; 32], required: 0,
///     granted: 0, plan: todo!(),
/// };
/// ```
///
/// A raw witness cannot be substituted for production admission:
///
/// ```compile_fail
/// use anomalous_program_flow::{admitted_program, conformance::checked_edition};
/// fn substitute<'a, 'w>(raw: checked_edition<'a, 'w>) -> admitted_program<'a, 'w> {
///     raw
/// }
/// ```
pub struct checked_edition<'a, 'w> {
    edition: &'a [u8],
    identity: [u8; 32],
    required: u64,
    granted: u64,
    plan: &'w program<'a>,
}

impl<'a> checked_edition<'a, '_> {
    pub const fn edition(&self) -> &'a [u8] {
        self.edition
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn intent(&self) -> Option<&'a str> {
        if self.plan.intent.is_empty() {
            None
        } else {
            Some(self.plan.intent)
        }
    }
    pub const fn required_services(&self) -> u64 {
        self.required
    }
    pub const fn granted_services(&self) -> u64 {
        self.granted
    }
    pub const fn program(&self) -> &program<'a> {
        self.plan
    }
}

/// Validate raw bytes using the existing bounded scalar grammar and an explicit
/// trusted effect ceiling. This operation cannot publish a production witness.
///
/// A live witness prevents reuse of the validation workspace:
///
/// ```compile_fail
/// use anomalous_program_flow::{conformance::validate_scalar, validation_workspace};
/// fn overwrite<'a>(source: &'a [u8], workspace: &mut validation_workspace<'a>) {
///     let first = validate_scalar(source, 0, workspace).unwrap();
///     let second = validate_scalar(source, 0, workspace).unwrap();
///     assert_eq!(first.identity(), second.identity());
/// }
/// ```
pub fn validate_scalar<'a, 'w>(
    edition: &'a [u8],
    ceiling: u64,
    workspace: &'w mut validation_workspace<'a>,
) -> Result<checked_edition<'a, 'w>, error> {
    workspace.reset();
    if ceiling & !authority::known != 0 {
        return Err(error::ceiling);
    }
    workspace.parse(
        edition,
        parse_profile::scalar,
        validation_purpose::conformance,
    )?;
    let required = workspace.plan.authority;
    if required & !ceiling != 0 {
        return Err(error::ceiling);
    }
    Ok(checked_edition {
        edition,
        identity: sha256(edition),
        required,
        granted: required & ceiling,
        plan: &workspace.plan,
    })
}

#[cfg(test)]
mod tests;
