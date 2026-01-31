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

/// Parse numeric value with suffix, returning (value, unit).
#[inline]
pub(crate) fn parse_with_suffix<T: Copy>(s: &str, suffix_len: usize, unit: T) -> Option<(u64, T)> {
    s[..s.len() - suffix_len]
        .parse::<u64>()
        .ok()
        .map(|v| (v, unit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_int_skip_underscores() {
        assert_eq!(parse_int_skip_underscores("123", 10), Some(123));
        assert_eq!(parse_int_skip_underscores("1_000_000", 10), Some(1_000_000));
        assert_eq!(parse_int_skip_underscores("1_2_3", 10), Some(123));
        assert_eq!(parse_int_skip_underscores("___1___", 10), Some(1));
    }

    #[test]
    fn test_parse_int_hex_with_underscores() {
        assert_eq!(parse_int_skip_underscores("FF", 16), Some(255));
        assert_eq!(parse_int_skip_underscores("F_F", 16), Some(255));
        assert_eq!(
            parse_int_skip_underscores("dead_beef", 16),
            Some(0xdead_beef)
        );
    }

    #[test]
    fn test_parse_int_binary_with_underscores() {
        assert_eq!(parse_int_skip_underscores("1010", 2), Some(10));
        assert_eq!(parse_int_skip_underscores("1_0_1_0", 2), Some(10));
        assert_eq!(parse_int_skip_underscores("1111_0000", 2), Some(240));
    }

    #[test]
    fn test_parse_int_overflow() {
        // Should return None on overflow
        assert_eq!(
            parse_int_skip_underscores("99999999999999999999999", 10),
            None
        );
    }

    #[test]
    #[allow(clippy::approx_constant)] // Testing float parsing, not using mathematical constants
    fn test_parse_float_skip_underscores() {
        assert_eq!(parse_float_skip_underscores("3.14"), Some(3.14));
        assert_eq!(parse_float_skip_underscores("1_000.5"), Some(1000.5));
        assert_eq!(parse_float_skip_underscores("1.5e10"), Some(1.5e10));
    }
}
