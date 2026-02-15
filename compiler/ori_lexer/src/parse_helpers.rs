//! Numeric Parsing Helpers
//!
//! Zero-allocation parsing utilities for numeric literals with underscore separators.

/// Parse integer skipping underscores without allocation.
#[inline]
pub(crate) fn parse_int_skip_underscores(s: &str, radix: u32) -> Option<u64> {
    let mut result: u64 = 0;
    for c in s.chars() {
        if c == '_' {
            continue;
        }
        let digit = c.to_digit(radix)?;
        result = result.checked_mul(u64::from(radix))?;
        result = result.checked_add(u64::from(digit))?;
    }
    Some(result)
}

/// Parse float - only allocate if underscores present.
#[inline]
pub(crate) fn parse_float_skip_underscores(s: &str) -> Option<f64> {
    if s.contains('_') {
        s.replace('_', "").parse().ok()
    } else {
        s.parse().ok()
    }
}

#[cfg(test)]
mod tests;
