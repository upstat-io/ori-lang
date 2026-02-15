use super::*;

#[test]
fn test_ranges_overlap() {
    // Overlapping
    assert!(ranges_overlap(0, 10, 5, 15));
    assert!(ranges_overlap(5, 15, 0, 10));
    assert!(ranges_overlap(0, 10, 0, 10)); // Same range

    // Contained
    assert!(ranges_overlap(0, 20, 5, 15));
    assert!(ranges_overlap(5, 15, 0, 20));

    // Not overlapping
    assert!(!ranges_overlap(0, 10, 10, 20)); // Adjacent
    assert!(!ranges_overlap(0, 10, 15, 20)); // Gap

    // Edge cases
    assert!(!ranges_overlap(0, 0, 0, 0)); // Empty ranges
    assert!(!ranges_overlap(5, 5, 5, 5)); // Empty ranges at same point
}

#[test]
fn test_apply_regions_single() {
    let source = "hello world";
    let regions = vec![FormattedRegion {
        original_start: 0,
        original_end: 5,
        formatted: "goodbye".to_string(),
    }];

    assert_eq!(apply_regions(source, regions), "goodbye world");
}

#[test]
fn test_apply_regions_multiple() {
    let source = "aaa bbb ccc";
    let regions = vec![
        FormattedRegion {
            original_start: 0,
            original_end: 3,
            formatted: "XXX".to_string(),
        },
        FormattedRegion {
            original_start: 8,
            original_end: 11,
            formatted: "ZZZ".to_string(),
        },
    ];

    assert_eq!(apply_regions(source, regions), "XXX bbb ZZZ");
}

#[test]
fn test_apply_regions_empty() {
    let source = "hello world";
    let regions = vec![];
    assert_eq!(apply_regions(source, regions), "hello world");
}

#[test]
fn test_apply_regions_size_change() {
    let source = "short text";
    let regions = vec![FormattedRegion {
        original_start: 0,
        original_end: 5,
        formatted: "very long replacement".to_string(),
    }];

    assert_eq!(apply_regions(source, regions), "very long replacement text");
}
