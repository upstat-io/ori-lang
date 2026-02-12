//! Width calculation for compound literal types.
//!
//! Provides width calculations for compound literals:
//! - Duration literals (100ms, 5s, 30m, 2h)
//! - Size literals (1024b, 4kb, 10mb, 2gb)

use ori_ir::{DurationUnit, SizeUnit};

use super::helpers::decimal_digit_count;

/// Calculate width of a duration literal.
///
/// Width is the number of digits plus the unit suffix length:
/// - `ns`, `us`, `ms` = 2 characters
/// - `s`, `m`, `h` = 1 character
#[expect(
    clippy::match_same_arms,
    reason = "Each arm explicitly documents the unit suffix for maintainability"
)]
pub(super) fn duration_width(value: u64, unit: DurationUnit) -> usize {
    let value_w = decimal_digit_count(value);

    let unit_w = match unit {
        DurationUnit::Nanoseconds => 2,  // "ns"
        DurationUnit::Microseconds => 2, // "us"
        DurationUnit::Milliseconds => 2, // "ms"
        DurationUnit::Seconds => 1,      // "s"
        DurationUnit::Minutes => 1,      // "m"
        DurationUnit::Hours => 1,        // "h"
    };

    value_w + unit_w
}

/// Calculate width of a size literal.
///
/// Width is the number of digits plus the unit suffix length:
/// - `b` = 1 character
/// - `kb`, `mb`, `gb`, `tb` = 2 characters
#[expect(
    clippy::match_same_arms,
    reason = "Each arm explicitly documents the unit suffix for maintainability"
)]
pub(super) fn size_width(value: u64, unit: SizeUnit) -> usize {
    let value_w = decimal_digit_count(value);

    let unit_w = match unit {
        SizeUnit::Bytes => 1,     // "b"
        SizeUnit::Kilobytes => 2, // "kb"
        SizeUnit::Megabytes => 2, // "mb"
        SizeUnit::Gigabytes => 2, // "gb"
        SizeUnit::Terabytes => 2, // "tb"
    };

    value_w + unit_w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_width_milliseconds() {
        assert_eq!(duration_width(100, DurationUnit::Milliseconds), 5); // "100ms"
        assert_eq!(duration_width(1, DurationUnit::Milliseconds), 3); // "1ms"
        assert_eq!(duration_width(0, DurationUnit::Milliseconds), 3); // "0ms"
    }

    #[test]
    fn test_duration_width_seconds() {
        assert_eq!(duration_width(5, DurationUnit::Seconds), 2); // "5s"
        assert_eq!(duration_width(60, DurationUnit::Seconds), 3); // "60s"
    }

    #[test]
    fn test_duration_width_minutes() {
        assert_eq!(duration_width(30, DurationUnit::Minutes), 3); // "30m"
    }

    #[test]
    fn test_duration_width_hours() {
        assert_eq!(duration_width(2, DurationUnit::Hours), 2); // "2h"
        assert_eq!(duration_width(24, DurationUnit::Hours), 3); // "24h"
    }

    #[test]
    fn test_size_width_bytes() {
        assert_eq!(size_width(1024, SizeUnit::Bytes), 5); // "1024b"
        assert_eq!(size_width(0, SizeUnit::Bytes), 2); // "0b"
    }

    #[test]
    fn test_size_width_kilobytes() {
        assert_eq!(size_width(4, SizeUnit::Kilobytes), 3); // "4kb"
    }

    #[test]
    fn test_size_width_megabytes() {
        assert_eq!(size_width(10, SizeUnit::Megabytes), 4); // "10mb"
    }

    #[test]
    fn test_size_width_gigabytes() {
        assert_eq!(size_width(2, SizeUnit::Gigabytes), 3); // "2gb"
    }
}
