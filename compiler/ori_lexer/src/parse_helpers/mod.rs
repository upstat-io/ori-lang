//! Numeric Parsing Helpers
//!
//! Zero-allocation parsing utilities for numeric literals with underscore separators.

/// Parse integer skipping underscores without allocation.
///
/// Returns `None` if no actual digits are present (empty string,
/// underscores-only) or on overflow.
#[inline]
pub(crate) fn parse_int_skip_underscores(s: &str, radix: u32) -> Option<u64> {
    let mut result: u64 = 0;
    let mut seen_digit = false;
    for c in s.chars() {
        if c == '_' {
            continue;
        }
        let digit = c.to_digit(radix)?;
        seen_digit = true;
        result = result.checked_mul(u64::from(radix))?;
        result = result.checked_add(u64::from(digit))?;
    }
    if seen_digit {
        Some(result)
    } else {
        None
    }
}

/// Parse float - only allocate if underscores present.
///
/// Returns `None` if the string can't be parsed or if the result
/// overflows to infinity (spec 7.7.2: overflow is a compile-time error).
#[inline]
pub(crate) fn parse_float_skip_underscores(s: &str) -> Option<f64> {
    let f: f64 = if s.contains('_') {
        s.replace('_', "").parse().ok()?
    } else {
        s.parse().ok()?
    };
    if f.is_finite() {
        Some(f)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
