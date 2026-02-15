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
mod tests;
