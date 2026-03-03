//! Duration and size literal cooking: suffix detection, decimal parsing, value validation.

use ori_ir::{DurationUnit, SizeUnit, TokenKind};

use super::{slice_source, span, CookResult, TokenCooker};
use crate::lex_error::LexError;
use crate::parse_helpers::parse_int_skip_underscores;

impl TokenCooker<'_> {
    pub(super) fn cook_duration(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);

        // Detect suffix by matching from the end
        let (suffix_len, unit) = detect_duration_suffix(text);
        if suffix_len == 0 {
            // Shouldn't happen with valid raw tokens, but be safe
            self.errors.push(LexError::int_overflow(span(offset, len)));
            return CookResult::with_error(TokenKind::Error);
        }

        let num_part = &text[..text.len() - suffix_len];

        if num_part.contains('.') {
            // Decimal duration: convert to nanoseconds via integer arithmetic.
            // Spec: "Decimal syntax is compile-time sugar computed via integer
            // arithmetic — no floating-point operations are involved."
            if let Some(nanos) = parse_decimal_unit_value(num_part, unit.nanos_multiplier()) {
                // Decimal nanos are already base units; validate i64 bounds.
                if i64::try_from(nanos).is_ok() {
                    CookResult::new(TokenKind::Duration(nanos, DurationUnit::Nanoseconds))
                } else {
                    self.errors.push(LexError::int_overflow(span(offset, len)));
                    CookResult::with_error(TokenKind::Error)
                }
            } else {
                self.errors
                    .push(LexError::decimal_not_representable(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
            }
        } else if let Some(value) = parse_int_skip_underscores(num_part, 10) {
            // Validate: value * nanos_multiplier must fit i64
            if unit.to_nanos(value).is_some() {
                CookResult::new(TokenKind::Duration(value, unit))
            } else {
                self.errors.push(LexError::int_overflow(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
            }
        } else {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
    }

    pub(super) fn cook_size(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);

        let (suffix_len, unit) = detect_size_suffix(text);
        if suffix_len == 0 {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            return CookResult::with_error(TokenKind::Error);
        }

        let num_part = &text[..text.len() - suffix_len];

        if num_part.contains('.') {
            // Decimal size: convert to bytes via integer arithmetic.
            if let Some(bytes) = parse_decimal_unit_value(num_part, unit.bytes_multiplier()) {
                // LLVM codegen casts bytes to i64; validate i64 bounds.
                if i64::try_from(bytes).is_ok() {
                    CookResult::new(TokenKind::Size(bytes, SizeUnit::Bytes))
                } else {
                    self.errors.push(LexError::int_overflow(span(offset, len)));
                    CookResult::with_error(TokenKind::Error)
                }
            } else {
                self.errors
                    .push(LexError::decimal_not_representable(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
            }
        } else if let Some(value) = parse_int_skip_underscores(num_part, 10) {
            // Validate: value * bytes_multiplier must not overflow AND fit i64.
            if unit
                .to_bytes(value)
                .is_some_and(|b| i64::try_from(b).is_ok())
            {
                CookResult::new(TokenKind::Size(value, unit))
            } else {
                self.errors.push(LexError::int_overflow(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
            }
        } else {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
    }
}

/// Parse a decimal number string and convert to base units using integer arithmetic.
///
/// Given `num_part` (e.g., `"1.5"`) and `multiplier` (e.g., `1_000_000_000` for seconds→ns),
/// computes the exact integer result. Returns `None` if the result is not a whole number
/// (e.g., `1.5` nanoseconds) or on overflow.
///
/// Algorithm: parse integer and fractional parts separately, then combine:
///   `result = integer_part * multiplier + fractional_digits * multiplier / 10^(num_frac_digits)`
///
/// The fractional contribution must divide evenly (no remainder) to be representable.
pub(crate) fn parse_decimal_unit_value(num_part: &str, multiplier: u64) -> Option<u64> {
    let mut integer_part: u64 = 0;
    let mut frac_digits: u64 = 0;
    let mut frac_digit_count: u32 = 0;
    let mut in_fraction = false;

    for &byte in num_part.as_bytes() {
        match byte {
            b'0'..=b'9' => {
                let digit = u64::from(byte - b'0');
                if in_fraction {
                    frac_digits = frac_digits.checked_mul(10)?.checked_add(digit)?;
                    frac_digit_count += 1;
                } else {
                    integer_part = integer_part.checked_mul(10)?.checked_add(digit)?;
                }
            }
            b'.' => {
                in_fraction = true;
            }
            b'_' => {}  // skip underscores
            _ => break, // hit suffix (shouldn't happen — caller strips suffix)
        }
    }

    // integer_contribution = integer_part * multiplier
    let integer_contribution = integer_part.checked_mul(multiplier)?;

    if frac_digit_count == 0 {
        return Some(integer_contribution);
    }

    // frac_divisor = 10^frac_digit_count
    let frac_divisor = 10u64.checked_pow(frac_digit_count)?;

    // frac_contribution = frac_digits * multiplier / frac_divisor
    // Must divide evenly for the result to be a whole number of base units.
    let frac_numerator = frac_digits.checked_mul(multiplier)?;
    if frac_numerator % frac_divisor != 0 {
        return None; // not representable as whole base units
    }
    let frac_contribution = frac_numerator / frac_divisor;

    integer_contribution.checked_add(frac_contribution)
}

/// Detect duration suffix and return (`suffix_len`, unit).
pub(crate) fn detect_duration_suffix(text: &str) -> (usize, DurationUnit) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    if n >= 2 {
        match (bytes[n - 2], bytes[n - 1]) {
            (b'n', b's') => return (2, DurationUnit::Nanoseconds),
            (b'u', b's') => return (2, DurationUnit::Microseconds),
            (b'm', b's') => return (2, DurationUnit::Milliseconds),
            _ => {}
        }
    }
    if n >= 1 {
        match bytes[n - 1] {
            b's' => return (1, DurationUnit::Seconds),
            b'm' => return (1, DurationUnit::Minutes),
            b'h' => return (1, DurationUnit::Hours),
            _ => {}
        }
    }
    (0, DurationUnit::Seconds)
}

/// Detect size suffix and return (`suffix_len`, unit).
pub(crate) fn detect_size_suffix(text: &str) -> (usize, SizeUnit) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    if n >= 2 {
        match (bytes[n - 2], bytes[n - 1]) {
            (b'k', b'b') => return (2, SizeUnit::Kilobytes),
            (b'm', b'b') => return (2, SizeUnit::Megabytes),
            (b'g', b'b') => return (2, SizeUnit::Gigabytes),
            (b't', b'b') => return (2, SizeUnit::Terabytes),
            _ => {}
        }
    }
    if n >= 1 && bytes[n - 1] == b'b' {
        return (1, SizeUnit::Bytes);
    }
    (0, SizeUnit::Bytes)
}
