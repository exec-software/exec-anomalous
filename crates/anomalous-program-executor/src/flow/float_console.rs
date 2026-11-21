use crate::error;
use core::fmt::{self, Write};

/// Fixed storage for six significant digits, a sign and a binary64 exponent.
pub(super) struct decimal {
    bytes: [u8; 16],
    length: usize,
}

impl decimal {
    const fn new() -> Self {
        Self {
            bytes: [0; 16],
            length: 0,
        }
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

impl Write for decimal {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length + text.len();
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

pub(super) fn format(bits: u64) -> Result<decimal, error> {
    let number = f64::from_bits(bits);
    if !number.is_finite() {
        return Err(error::invalid_state);
    }
    general(number).map_err(|_| error::invalid_state)
}

fn general(number: f64) -> Result<decimal, fmt::Error> {
    // Core's binary64 formatting rounds the significand to nearest, ties to
    // even. Use the rounded exponent for the classic general-format cutoff.
    let mut scientific = decimal::new();
    write!(scientific, "{:.5e}", number.abs())?;
    let encoded = core::str::from_utf8(scientific.bytes()).map_err(|_| fmt::Error)?;
    let (coefficient, exponent) = encoded.split_once('e').ok_or(fmt::Error)?;
    let exponent: i32 = exponent.parse().map_err(|_| fmt::Error)?;
    let (head, tail) = coefficient.split_once('.').ok_or(fmt::Error)?;
    let tail = tail.trim_end_matches('0');

    let mut output = decimal::new();
    if number.is_sign_negative() {
        output.write_char('-')?;
    }
    match exponent {
        -4..=-1 => {
            output.write_str("0.")?;
            for _ in 0..(-exponent - 1) {
                output.write_char('0')?;
            }
            output.write_str(head)?;
            output.write_str(tail)?;
        }
        0..=5 => {
            let integer_digits = exponent as usize;
            let (whole, fraction) = tail.split_at(integer_digits.min(tail.len()));
            output.write_str(head)?;
            output.write_str(whole)?;
            for _ in whole.len()..integer_digits {
                output.write_char('0')?;
            }
            if !fraction.is_empty() {
                output.write_char('.')?;
                output.write_str(fraction)?;
            }
        }
        _ => {
            output.write_str(head)?;
            if !tail.is_empty() {
                output.write_char('.')?;
                output.write_str(tail)?;
            }
            let sign = if exponent < 0 { '-' } else { '+' };
            write!(output, "e{sign}{:02}", exponent.unsigned_abs())?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_values_round_before_selecting_notation() {
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "-0"),
            (f64::from_bits(1), "4.94066e-324"),
            (f64::MIN_POSITIVE, "2.22507e-308"),
            (f64::MAX, "1.79769e+308"),
            (f64::MIN, "-1.79769e+308"),
            (0.000_099_999_99, "0.0001"),
            (0.000_099_999_94, "9.99999e-05"),
            (999_999.4, "999999"),
            (999_999.5, "1e+06"),
            (100_000.0, "100000"),
            (123_456.5, "123456"),
            (123_457.5, "123458"),
            (-0.000_123_456_5, "-0.000123457"),
        ] {
            let actual = format(value.to_bits()).unwrap();
            assert_eq!(actual.bytes(), expected.as_bytes(), "{value:?}");
        }
    }

    #[test]
    fn rejects_nonfinite_bits() {
        for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert_eq!(format(value.to_bits()).err(), Some(error::invalid_state));
        }
    }
}
