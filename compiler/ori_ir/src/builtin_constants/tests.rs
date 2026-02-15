use super::*;

#[test]
fn test_duration_constants() {
    assert_eq!(duration::NS_PER_US, 1_000);
    assert_eq!(duration::NS_PER_MS, 1_000_000);
    assert_eq!(duration::NS_PER_S, 1_000_000_000);
    assert_eq!(duration::NS_PER_M, 60_000_000_000);
    assert_eq!(duration::NS_PER_H, 3_600_000_000_000);
}

#[test]
fn test_size_constants() {
    // SI units: powers of 1000
    assert_eq!(size::BYTES_PER_KB, 1_000);
    assert_eq!(size::BYTES_PER_MB, 1_000_000);
    assert_eq!(size::BYTES_PER_GB, 1_000_000_000);
    assert_eq!(size::BYTES_PER_TB, 1_000_000_000_000);
}

#[test]
fn test_ordering_constants() {
    assert_eq!(ordering::LESS, 0);
    assert_eq!(ordering::EQUAL, 1);
    assert_eq!(ordering::GREATER, 2);
}
