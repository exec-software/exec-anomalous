//! Explicit scalar conversions. Floating-point values are opaque finite bits;
//! arithmetic, numeric coercion and text formatting require separate profiles.

use super::{error, narrow, value};
use anomalous_program_flow::scalar_cast;

pub(super) fn cast(item: value<'_>, target: scalar_cast) -> Result<value<'_>, error> {
    if target == scalar_cast::float_identity {
        return match item {
            value::float(bits) if bits & 0x7ff0_0000_0000_0000 != 0x7ff0_0000_0000_0000 => {
                Ok(value::float(bits))
            }
            _ => Err(error::invalid_state),
        };
    }
    let number = match item {
        value::integer(number) => number,
        value::boolean(truth) => i64::from(truth),
        value::byte(byte) => i64::from(byte),
        _ => return Err(error::invalid_state),
    };
    match target {
        scalar_cast::boolean => match number {
            0 => Ok(value::boolean(false)),
            1 => Ok(value::boolean(true)),
            _ => Err(error::scalar_conversion),
        },
        scalar_cast::byte => Ok(value::byte((number as u64 & 0xff) as u8)),
        scalar_cast::integer32 => Ok(value::integer(narrow(number, "i32")?)),
        scalar_cast::integer64 => Ok(value::integer(number)),
        scalar_cast::float_identity => Err(error::invalid_state),
    }
}
