//! Shared width calculation utilities.
//!
//! This module provides common helper functions used across the width calculation
//! system: digit counting for numeric literals and accumulation of widths with
//! separators for list-like structures.

use super::ALWAYS_STACKED;

/// Separator width for comma-separated items: ", " = 2 characters.
pub(super) const COMMA_SEPARATOR_WIDTH: usize = 2;

/// Count decimal digits in a non-negative integer.
///
/// Returns the number of digits needed to represent `n` in base 10.
/// For `n == 0`, returns 1 (representing "0").
#[inline]
pub(super) fn decimal_digit_count(n: u64) -> usize {
    if n == 0 {
        return 1;
    }
    (n.ilog10() + 1) as usize
}

/// Accumulate widths with a separator between items.
///
/// Returns `ALWAYS_STACKED` if any item's width is `ALWAYS_STACKED`,
/// ensuring that stacked constructs propagate through containers.
///
/// # Arguments
///
/// * `items` - The items to measure
/// * `get_width` - Function to get the width of each item
/// * `separator_width` - Width of separator between items (e.g., 2 for ", ")
pub(super) fn accumulate_widths<T, F>(items: &[T], mut get_width: F, separator_width: usize) -> usize
where
    F: FnMut(&T) -> usize,
{
    if items.is_empty() {
        return 0;
    }

    let mut total = 0;
    for (i, item) in items.iter().enumerate() {
        let w = get_width(item);
        if w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        total += w;
        if i < items.len() - 1 {
            total += separator_width;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimal_digit_count_zero() {
        assert_eq!(decimal_digit_count(0), 1);
    }

    #[test]
    fn test_decimal_digit_count_single_digit() {
        assert_eq!(decimal_digit_count(1), 1);
        assert_eq!(decimal_digit_count(9), 1);
    }

    #[test]
    fn test_decimal_digit_count_multi_digit() {
        assert_eq!(decimal_digit_count(10), 2);
        assert_eq!(decimal_digit_count(99), 2);
        assert_eq!(decimal_digit_count(100), 3);
        assert_eq!(decimal_digit_count(999), 3);
        assert_eq!(decimal_digit_count(1000), 4);
        assert_eq!(decimal_digit_count(123_456), 6);
    }

    #[test]
    fn test_decimal_digit_count_large() {
        assert_eq!(decimal_digit_count(u64::MAX), 20); // 18446744073709551615
    }

    #[test]
    fn test_accumulate_widths_empty() {
        let items: Vec<usize> = vec![];
        assert_eq!(accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH), 0);
    }

    #[test]
    fn test_accumulate_widths_single() {
        let items = vec![5];
        assert_eq!(accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH), 5);
    }

    #[test]
    fn test_accumulate_widths_multiple() {
        let items = vec![1, 2, 3];
        // 1 + 2 + 2 + 2 + 3 = 10 (two separators of width 2)
        assert_eq!(accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH), 10);
    }

    #[test]
    fn test_accumulate_widths_always_stacked_propagation() {
        let items = vec![1, ALWAYS_STACKED, 3];
        assert_eq!(
            accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH),
            ALWAYS_STACKED
        );
    }
}
