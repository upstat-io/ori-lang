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
